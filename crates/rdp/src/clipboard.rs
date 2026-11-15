//! Text clipboard sharing over the CLIPRDR channel.
//!
//! The channel's callbacks run while the session processes server data, so the
//! backend only writes down what happened. The session thread reads the notes
//! afterwards and does the sending.

use ironrdp_cliprdr::backend::CliprdrBackend;
use ironrdp_cliprdr::pdu::{
    ClipboardFormatId, ClipboardGeneralCapabilityFlags, FileContentsRequest, FileContentsResponse,
    FormatDataRequest, FormatDataResponse, LockDataId,
};
use ironrdp_cliprdr::{Client, Cliprdr};
use ironrdp_core::impl_as_any;

/// What happened since the notes were last taken.
#[derive(Debug, Default)]
pub(crate) struct Notes {
    /// The server copied text, which can now be asked for.
    pub(crate) remote_has_text: bool,
    /// Text the server sent after being asked.
    pub(crate) received: Vec<String>,
    /// The server wants our text.
    pub(crate) text_requested: bool,
    /// The server wants something else, which is answered with a refusal.
    pub(crate) other_requested: bool,
    /// The channel is ready, so our clipboard can be announced.
    pub(crate) ready: bool,
}

/// The channel's side of the clipboard: collects notes, shares only text.
#[derive(Debug, Default)]
pub(crate) struct TextBackend {
    notes: Notes,
}

impl_as_any!(TextBackend);

impl TextBackend {
    pub(crate) fn take_notes(&mut self) -> Notes {
        std::mem::take(&mut self.notes)
    }
}

/// The notes of the clipboard channel in `channel`, if it has any.
pub(crate) fn take_notes(channel: &mut Cliprdr<Client>) -> Notes {
    channel
        .downcast_backend_mut::<TextBackend>()
        .map(TextBackend::take_notes)
        .unwrap_or_default()
}

/// The channel with a text backend, ready to be attached to a connection.
pub(crate) fn channel() -> Cliprdr<Client> {
    Cliprdr::new(Box::new(TextBackend::default()))
}

impl CliprdrBackend for TextBackend {
    fn temporary_directory(&self) -> &str {
        // Only used for file transfers, which are not offered.
        ""
    }

    fn client_capabilities(&self) -> ClipboardGeneralCapabilityFlags {
        ClipboardGeneralCapabilityFlags::empty()
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
        if available_formats
            .iter()
            .any(|format| format.id == ClipboardFormatId::CF_UNICODETEXT)
        {
            self.notes.remote_has_text = true;
        }
    }

    fn on_format_data_request(&mut self, request: FormatDataRequest) {
        if request.format == ClipboardFormatId::CF_UNICODETEXT {
            self.notes.text_requested = true;
        } else {
            self.notes.other_requested = true;
        }
    }

    fn on_format_data_response(&mut self, response: FormatDataResponse<'_>) {
        if response.is_error() {
            return;
        }
        if let Ok(text) = response.to_unicode_string() {
            self.notes.received.push(text);
        }
    }

    fn on_file_contents_request(&mut self, _request: FileContentsRequest) {}

    fn on_file_contents_response(&mut self, _response: FileContentsResponse<'_>) {}

    fn on_lock(&mut self, _data_id: LockDataId) {}

    fn on_unlock(&mut self, _data_id: LockDataId) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironrdp_cliprdr::pdu::ClipboardFormat;

    #[test]
    fn copied_text_on_the_server_is_noted() {
        let mut backend = TextBackend::default();

        backend.on_remote_copy(&[ClipboardFormat::new(ClipboardFormatId::CF_UNICODETEXT)]);

        assert!(backend.take_notes().remote_has_text);
    }

    #[test]
    fn other_formats_on_the_server_are_ignored() {
        let mut backend = TextBackend::default();

        backend.on_remote_copy(&[ClipboardFormat::new(ClipboardFormatId::CF_DIB)]);

        assert!(!backend.take_notes().remote_has_text);
    }

    #[test]
    fn a_request_for_text_is_told_apart_from_other_requests() {
        let mut backend = TextBackend::default();

        backend.on_format_data_request(FormatDataRequest {
            format: ClipboardFormatId::CF_UNICODETEXT,
        });
        backend.on_format_data_request(FormatDataRequest {
            format: ClipboardFormatId::CF_DIB,
        });

        let notes = backend.take_notes();
        assert!(notes.text_requested && notes.other_requested);
    }

    #[test]
    fn received_text_is_decoded() {
        let mut backend = TextBackend::default();
        let response = FormatDataResponse::new_unicode_string("zażółć");

        backend.on_format_data_response(response);

        assert_eq!(backend.take_notes().received, ["zażółć"]);
    }

    #[test]
    fn an_error_response_adds_nothing() {
        let mut backend = TextBackend::default();

        backend.on_format_data_response(FormatDataResponse::new_error());

        assert!(backend.take_notes().received.is_empty());
    }

    #[test]
    fn taking_the_notes_clears_them() {
        let mut backend = TextBackend::default();
        backend.on_ready();

        assert!(backend.take_notes().ready);
        assert!(!backend.take_notes().ready);
    }
}
