//! Text and picture clipboard sharing over the CLIPRDR channel.
//!
//! The channel's callbacks run while the session processes server data, so the
//! backend only writes down what happened. The session thread reads the notes
//! afterwards and does the sending.

use ironrdp_cliprdr::backend::CliprdrBackend;
use ironrdp_cliprdr::pdu::{
    ClipboardFormat, ClipboardFormatId, ClipboardFormatName, ClipboardGeneralCapabilityFlags,
    FileContentsRequest, FileContentsResponse, FileDescriptor, FormatDataRequest,
    FormatDataResponse, LockDataId,
};
use ironrdp_cliprdr::{Client, Cliprdr};
use ironrdp_core::{IntoOwned, impl_as_any};
use ironrdp_session::ActiveStage;

use crate::files::{FileNotes, Files, FilesEvent};

/// What a pasted piece of clipboard data was asked for, so the answer can be read right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Text,
    Picture,
}

/// What happened since the notes were last taken.
#[derive(Debug, Default)]
pub(crate) struct Notes {
    /// The server copied text, which can now be asked for.
    pub(crate) remote_has_text: bool,
    /// The server copied a picture, which can now be asked for.
    pub(crate) remote_has_picture: bool,
    /// Text the server sent after being asked.
    pub(crate) received: Vec<String>,
    /// Pictures (as DIB data) the server sent after being asked.
    pub(crate) received_pictures: Vec<Vec<u8>>,
    /// The server wants our text.
    pub(crate) text_requested: bool,
    /// The server wants our picture.
    pub(crate) picture_requested: bool,
    /// The server wants something else, which is answered with a refusal.
    pub(crate) other_requested: bool,
    /// The channel is ready, so our clipboard can be announced.
    pub(crate) ready: bool,
    /// What happened to files.
    pub(crate) files: FileNotes,
}

/// The channel's side of the clipboard: collects notes, shares text and pictures.
#[derive(Debug, Default)]
pub(crate) struct ClipboardBackend {
    notes: Notes,
    /// What each unanswered paste asked for, oldest first.
    awaiting: std::collections::VecDeque<Kind>,
}

impl_as_any!(ClipboardBackend);

impl ClipboardBackend {
    pub(crate) fn take_notes(&mut self) -> Notes {
        std::mem::take(&mut self.notes)
    }
}

/// The notes of the clipboard channel in `channel`, if it has any.
pub(crate) fn take_notes(channel: &mut Cliprdr<Client>) -> Notes {
    channel
        .downcast_backend_mut::<ClipboardBackend>()
        .map(ClipboardBackend::take_notes)
        .unwrap_or_default()
}

/// Remembers that a paste of `kind` was sent, so its answer is read as that kind.
pub(crate) fn expect(channel: &mut Cliprdr<Client>, kind: Kind) {
    if let Some(backend) = channel.downcast_backend_mut::<ClipboardBackend>() {
        backend.awaiting.push_back(kind);
    }
}

/// What the local clipboard holds, and what the channel has to do about it.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    text: Option<String>,
    /// A picture as DIB data, ready to be sent.
    picture: Option<Vec<u8>>,
    /// The local clipboard changed and the server has not been told.
    announce: bool,
    /// Files going either way.
    pub(crate) files: Files,
}

/// What came out of one round with the channel.
#[derive(Debug, Default)]
pub(crate) struct Served {
    /// Bytes to send to the server.
    pub(crate) outgoing: Vec<Vec<u8>>,
    /// Texts the server copied.
    pub(crate) texts: Vec<String>,
    /// Pictures the server copied, as DIB data.
    pub(crate) pictures: Vec<Vec<u8>>,
    /// What happened to files.
    pub(crate) files: Vec<FilesEvent>,
}

type BoxedError = Box<dyn std::error::Error>;

impl Shared {
    /// The local clipboard now holds `text` and nothing else.
    pub(crate) fn set_text(&mut self, text: String) {
        self.text = Some(text);
        self.picture = None;
        self.announce = true;
    }

    /// The local clipboard now holds this picture (as DIB data) and nothing else.
    pub(crate) fn set_picture(&mut self, dib: Vec<u8>) {
        self.picture = Some(dib);
        self.text = None;
        self.announce = true;
    }

    /// The formats the local clipboard can give.
    fn formats(&self) -> Vec<ClipboardFormat> {
        let mut formats = Vec::new();
        if self.text.is_some() {
            formats.push(ClipboardFormat::new(ClipboardFormatId::CF_UNICODETEXT));
        }
        if self.picture.is_some() {
            formats.push(ClipboardFormat::new(ClipboardFormatId::CF_DIB));
        }
        formats
    }

    /// Does what the channel's notes ask for, as far as the channel allows.
    pub(crate) fn serve(&mut self, stage: &mut ActiveStage) -> Result<Served, BoxedError> {
        let Some(channel) = stage.get_svc_processor_mut::<Cliprdr<Client>>() else {
            return Ok(Served::default());
        };
        let notes = take_notes(channel);
        let mut messages = Vec::new();
        // The channel starts by answering with what is on the clipboard, even if that is nothing.
        if notes.ready || (self.announce && channel.is_ready()) {
            messages.push(channel.initiate_copy(&self.formats())?);
            self.announce = false;
        }
        // Text wins when the server copied both, so a document does not turn into a picture.
        if notes.remote_has_text {
            expect(channel, Kind::Text);
            messages.push(channel.initiate_paste(ClipboardFormatId::CF_UNICODETEXT)?);
        } else if notes.remote_has_picture {
            expect(channel, Kind::Picture);
            messages.push(channel.initiate_paste(ClipboardFormatId::CF_DIB)?);
        }
        if notes.text_requested {
            let ours = self.text.clone().unwrap_or_default();
            let reply = FormatDataResponse::new_unicode_string(&ours).into_owned();
            messages.push(channel.submit_format_data(reply)?);
        }
        if notes.picture_requested {
            let reply = match &self.picture {
                Some(dib) => FormatDataResponse::new_data(dib.clone()).into_owned(),
                None => FormatDataResponse::new_error().into_owned(),
            };
            messages.push(channel.submit_format_data(reply)?);
        }
        if notes.other_requested {
            let refusal = FormatDataResponse::new_error().into_owned();
            messages.push(channel.submit_format_data(refusal)?);
        }
        let mut files = Vec::new();
        self.files
            .step(channel, notes.files, &mut messages, &mut files)?;
        let mut served = Served {
            texts: notes.received,
            pictures: notes.received_pictures,
            files,
            ..Served::default()
        };
        for one in messages {
            served
                .outgoing
                .push(stage.process_svc_processor_messages(one)?);
        }
        Ok(served)
    }
}

/// The channel with a clipboard backend, ready to be attached to a connection.
pub(crate) fn channel() -> Cliprdr<Client> {
    Cliprdr::new(Box::new(ClipboardBackend::default()))
}

impl CliprdrBackend for ClipboardBackend {
    fn temporary_directory(&self) -> &str {
        // Files are written where the user chose, never to a temporary place.
        ""
    }

    fn client_capabilities(&self) -> ClipboardGeneralCapabilityFlags {
        // File transfer needs long format names (the file list has a name), streamed
        // contents, and no paths: Windows sends names without drive letters.
        ClipboardGeneralCapabilityFlags::USE_LONG_FORMAT_NAMES
            | ClipboardGeneralCapabilityFlags::STREAM_FILECLIP_ENABLED
            | ClipboardGeneralCapabilityFlags::FILECLIP_NO_FILE_PATHS
            | ClipboardGeneralCapabilityFlags::CAN_LOCK_CLIPDATA
    }

    fn on_ready(&mut self) {
        self.notes.ready = true;
    }

    fn on_request_format_list(&mut self) {
        self.notes.ready = true;
    }

    fn on_process_negotiated_capabilities(
        &mut self,
        _capabilities: ClipboardGeneralCapabilityFlags,
    ) {
    }

    fn on_remote_copy(&mut self, available_formats: &[ironrdp_cliprdr::pdu::ClipboardFormat]) {
        let has =
            |wanted: ClipboardFormatId| available_formats.iter().any(|format| format.id == wanted);
        if has(ClipboardFormatId::CF_UNICODETEXT) {
            self.notes.remote_has_text = true;
        }
        if has(ClipboardFormatId::CF_DIB) {
            self.notes.remote_has_picture = true;
        }
        let list = available_formats
            .iter()
            .find(|format| format.name() == Some(&ClipboardFormatName::FILE_LIST));
        if let Some(list) = list {
            self.notes.files.remote_list = Some(list.id());
        }
    }

    fn on_remote_file_list(&mut self, files: &[FileDescriptor], clip_data_id: Option<u32>) {
        self.notes.files.list = Some((files.to_vec(), clip_data_id));
    }

    fn on_format_data_request(&mut self, request: FormatDataRequest) {
        if request.format == ClipboardFormatId::CF_UNICODETEXT {
            self.notes.text_requested = true;
        } else if request.format == ClipboardFormatId::CF_DIB {
            self.notes.picture_requested = true;
        } else {
            self.notes.other_requested = true;
        }
    }

    fn on_format_data_response(&mut self, response: FormatDataResponse<'_>) {
        // Answers come in the order the pastes were sent.
        let kind = self.awaiting.pop_front().unwrap_or(Kind::Text);
        if response.is_error() {
            return;
        }
        match kind {
            Kind::Text => {
                if let Ok(text) = response.to_unicode_string() {
                    self.notes.received.push(text);
                }
            }
            Kind::Picture => self.notes.received_pictures.push(response.data().to_vec()),
        }
    }

    fn on_file_contents_request(&mut self, request: FileContentsRequest) {
        self.notes.files.requests.push(request);
    }

    fn on_file_contents_response(&mut self, response: FileContentsResponse<'_>) {
        let data = (!response.is_error()).then(|| response.data().to_vec());
        self.notes
            .files
            .responses
            .push((response.stream_id(), data));
    }

    fn on_lock(&mut self, _data_id: LockDataId) {}

    fn on_unlock(&mut self, _data_id: LockDataId) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironrdp_cliprdr::pdu::ClipboardFormat;

    #[test]
    fn copied_text_on_the_server_is_noted() {
        let mut backend = ClipboardBackend::default();

        backend.on_remote_copy(&[ClipboardFormat::new(ClipboardFormatId::CF_UNICODETEXT)]);

        assert!(backend.take_notes().remote_has_text);
    }

    #[test]
    fn other_formats_on_the_server_are_ignored() {
        let mut backend = ClipboardBackend::default();

        backend.on_remote_copy(&[ClipboardFormat::new(ClipboardFormatId::CF_TIFF)]);

        assert!(!backend.take_notes().remote_has_text);
    }

    #[test]
    fn a_request_for_text_is_told_apart_from_other_requests() {
        let mut backend = ClipboardBackend::default();

        backend.on_format_data_request(FormatDataRequest {
            format: ClipboardFormatId::CF_UNICODETEXT,
        });
        backend.on_format_data_request(FormatDataRequest {
            format: ClipboardFormatId::CF_TIFF,
        });

        let notes = backend.take_notes();
        assert!(notes.text_requested && notes.other_requested);
    }

    #[test]
    fn a_picture_on_the_server_is_noted_apart_from_text() {
        let mut backend = ClipboardBackend::default();

        backend.on_remote_copy(&[ClipboardFormat::new(ClipboardFormatId::CF_DIB)]);

        let notes = backend.take_notes();
        assert!(notes.remote_has_picture && !notes.remote_has_text);
    }

    #[test]
    fn a_request_for_a_picture_is_told_apart() {
        let mut backend = ClipboardBackend::default();

        backend.on_format_data_request(FormatDataRequest {
            format: ClipboardFormatId::CF_DIB,
        });

        let notes = backend.take_notes();
        assert!(notes.picture_requested && !notes.other_requested);
    }

    #[test]
    fn an_answer_is_read_as_what_the_paste_asked_for() {
        let mut backend = ClipboardBackend::default();
        backend.awaiting.push_back(Kind::Picture);
        backend.awaiting.push_back(Kind::Text);

        backend.on_format_data_response(FormatDataResponse::new_data(vec![1, 2, 3]));
        backend.on_format_data_response(FormatDataResponse::new_unicode_string("zażółć"));

        let notes = backend.take_notes();
        assert_eq!(notes.received_pictures, [vec![1, 2, 3]]);
        assert_eq!(notes.received, ["zażółć"]);
    }

    #[test]
    fn received_text_is_decoded() {
        let mut backend = ClipboardBackend::default();
        let response = FormatDataResponse::new_unicode_string("zażółć");

        backend.on_format_data_response(response);

        assert_eq!(backend.take_notes().received, ["zażółć"]);
    }

    #[test]
    fn an_error_response_adds_nothing() {
        let mut backend = ClipboardBackend::default();

        backend.on_format_data_response(FormatDataResponse::new_error());

        assert!(backend.take_notes().received.is_empty());
    }

    #[test]
    fn copied_files_on_the_server_are_noted_by_the_name_of_their_format() {
        let mut backend = ClipboardBackend::default();
        let list = ClipboardFormat::new(ClipboardFormatId::new(0xC0F0))
            .with_name(ClipboardFormatName::FILE_LIST);

        backend.on_remote_copy(&[
            ClipboardFormat::new(ClipboardFormatId::CF_UNICODETEXT),
            list,
        ]);

        let notes = backend.take_notes();
        assert_eq!(
            notes.files.remote_list,
            Some(ClipboardFormatId::new(0xC0F0))
        );
    }

    #[test]
    fn a_file_answer_keeps_its_stream_and_a_failed_one_has_no_data() {
        let mut backend = ClipboardBackend::default();

        backend.on_file_contents_response(FileContentsResponse::new_data_response(4, vec![1, 2]));
        backend.on_file_contents_response(FileContentsResponse::new_error(5));

        let notes = backend.take_notes();
        assert_eq!(notes.files.responses, [(4, Some(vec![1, 2])), (5, None)]);
    }

    #[test]
    fn the_client_asks_for_file_transfer() {
        let flags = ClipboardBackend::default().client_capabilities();

        assert!(flags.contains(ClipboardGeneralCapabilityFlags::STREAM_FILECLIP_ENABLED));
        assert!(flags.contains(ClipboardGeneralCapabilityFlags::USE_LONG_FORMAT_NAMES));
    }

    #[test]
    fn taking_the_notes_clears_them() {
        let mut backend = ClipboardBackend::default();
        backend.on_ready();

        assert!(backend.take_notes().ready);
        assert!(!backend.take_notes().ready);
    }
}
