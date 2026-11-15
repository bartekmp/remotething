//! The session thread: reads the server, forwards input, passes frames on.

use std::io::ErrorKind;
use std::time::{Duration, Instant};

use ironrdp_cliprdr::pdu::{ClipboardFormat, ClipboardFormatId, FormatDataResponse};
use ironrdp_cliprdr::{Client, Cliprdr};
use ironrdp_connector::Config;
use ironrdp_connector::ConnectionResult;
use ironrdp_core::IntoOwned;
use ironrdp_displaycontrol::pdu::MonitorLayoutEntry;
use ironrdp_graphics::image_processing::PixelFormat;
use ironrdp_session::image::DecodedImage;
use ironrdp_session::{ActiveStage, ActiveStageBuilder, ActiveStageOutput};
use tokio::sync::mpsc;

use crate::clipboard;
use crate::input::{self, Input};
use crate::reactivate;
use crate::session::{Event, Frame, TlsFramed};
use crate::stats::Counter;

/// The most frames sent to the window per second.
const FRAME_INTERVAL: Duration = Duration::from_millis(33);

/// How long a session must have run before the server is asked to resize.
const RESIZE_AFTER: Duration = Duration::from_secs(2);

type BoxedError = Box<dyn std::error::Error>;

/// What the server's output asks of the loop.
enum Flow {
    Continue,
    /// The server ended the session.
    Terminate,
    /// The server is changing the desktop size.
    Reactivate,
}

pub(crate) struct Worker {
    stage: ActiveStage,
    image: DecodedImage,
    framed: TlsFramed,
    config: Config,
    channels: (u16, u16),
    counter: Counter,
    /// A picture was updated and has not been sent to the window.
    dirty: bool,
    last_sent: Instant,
    /// A size the window wants, kept until the server can be asked.
    resize: Option<(u16, u16, u32)>,
    started: Instant,
    /// The text on the local clipboard, for when the server pastes.
    local_text: Option<String>,
    /// The local clipboard changed and the server has not been told.
    announce: bool,
}

impl Worker {
    pub(crate) fn new(connection: ConnectionResult, framed: TlsFramed, config: Config) -> Self {
        let channels = (connection.io_channel_id, connection.user_channel_id);
        let image = new_image(
            connection.desktop_size.width,
            connection.desktop_size.height,
        );
        let stage = ActiveStageBuilder {
            static_channels: connection.static_channels,
            user_channel_id: connection.user_channel_id,
            io_channel_id: connection.io_channel_id,
            message_channel_id: connection.message_channel_id,
            share_id: connection.share_id,
            compression_type: connection.compression_type,
            enable_server_pointer: connection.enable_server_pointer,
            pointer_software_rendering: connection.pointer_software_rendering,
        }
        .build();
        Self {
            stage,
            image,
            framed,
            config,
            channels,
            counter: Counter::new(),
            dirty: false,
            last_sent: Instant::now(),
            resize: None,
            started: Instant::now(),
            local_text: None,
            announce: false,
        }
    }

    /// Runs until the server ends the session or the window drops it.
    pub(crate) fn run(
        mut self,
        events: &mpsc::Sender<Event>,
        mut input: mpsc::Receiver<Input>,
    ) -> Result<(), BoxedError> {
        loop {
            match self.read_server()? {
                Flow::Continue => {}
                Flow::Terminate => return Ok(()),
                Flow::Reactivate => self.reactivate()?,
            }
            if !self.forward_input(&mut input)? {
                // The window dropped the session.
                return Ok(());
            }
            self.ask_for_resize()?;
            self.share_clipboard(events)?;
            if !self.send_frame(events) {
                return Ok(());
            }
            if let Some(totals) = self.counter.due() {
                // A full queue only delays the numbers.
                let _ = events.try_send(Event::Stats(totals));
            }
        }
    }

    fn read_server(&mut self) -> Result<Flow, BoxedError> {
        match self.framed.read_pdu() {
            Ok((action, payload)) => {
                self.counter.received(payload.len());
                let received_at = self.framed.last_read_at();
                let outputs = self
                    .stage
                    .process(&mut self.image, action, &payload, received_at)?;
                Ok(self.handle(outputs)?)
            }
            // The read timeout lets input and a pending frame go out while the server is quiet.
            Err(error) if is_timeout(&error) => Ok(Flow::Continue),
            Err(error) => Err(error.into()),
        }
    }

    /// Writes what the session wants sent and says what the loop should do next.
    fn handle(&mut self, outputs: Vec<ActiveStageOutput>) -> std::io::Result<Flow> {
        let mut flow = Flow::Continue;
        for output in outputs {
            match output {
                ActiveStageOutput::ResponseFrame(response) => self.framed.write_all(&response)?,
                ActiveStageOutput::GraphicsUpdate(_) => self.dirty = true,
                ActiveStageOutput::Terminate(_) => return Ok(Flow::Terminate),
                ActiveStageOutput::DeactivateAll => flow = Flow::Reactivate,
                _ => {}
            }
        }
        Ok(flow)
    }

    /// Answers a size change by the server; the picture starts again at the new size.
    fn reactivate(&mut self) -> Result<(), BoxedError> {
        self.image = reactivate::run(
            &mut self.framed,
            &mut self.stage,
            &self.config,
            self.channels,
        )?;
        self.dirty = true;
        Ok(())
    }

    /// Sends the input that is waiting. False when the window is gone.
    fn forward_input(&mut self, input: &mut mpsc::Receiver<Input>) -> Result<bool, BoxedError> {
        let mut events = Vec::new();
        loop {
            match input.try_recv() {
                Ok(Input::Resize {
                    width,
                    height,
                    scale,
                }) => self.resize = Some((width, height, scale)),
                Ok(Input::Clipboard(text)) => {
                    self.local_text = Some(text);
                    self.announce = true;
                }
                Ok(one) => events.extend(input::to_events(one)),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => return Ok(false),
            }
        }
        if !events.is_empty() {
            let outputs = self
                .stage
                .process_fastpath_input(&mut self.image, &events)?;
            self.handle(outputs)?;
        }
        Ok(true)
    }

    /// Asks the server for the wanted size once the channel for it is ready.
    fn ask_for_resize(&mut self) -> Result<(), BoxedError> {
        let Some((width, height, scale)) = self.resize else {
            return Ok(());
        };
        if !resize_allowed(self.started.elapsed()) {
            return Ok(());
        }
        let (width, height) =
            MonitorLayoutEntry::adjust_display_size(u32::from(width), u32::from(height));
        // None: not ready yet, or the server has no such channel. Try again next round.
        if let Some(request) = self.stage.encode_resize(width, height, Some(scale), None) {
            self.framed.write_all(&request?)?;
            self.resize = None;
        }
        Ok(())
    }

    /// Carries text between the two clipboards, as far as the channel allows.
    fn share_clipboard(&mut self, events: &mpsc::Sender<Event>) -> Result<(), BoxedError> {
        let Some(channel) = self.stage.get_svc_processor_mut::<Cliprdr<Client>>() else {
            return Ok(());
        };
        let notes = clipboard::take_notes(channel);
        let mut outgoing = Vec::new();
        // The channel starts by answering with what is on the clipboard, even if that is nothing.
        if notes.ready || (self.announce && channel.is_ready()) {
            let formats = match self.local_text {
                Some(_) => vec![ClipboardFormat::new(ClipboardFormatId::CF_UNICODETEXT)],
                None => Vec::new(),
            };
            outgoing.push(channel.initiate_copy(&formats)?);
            self.announce = false;
        }
        if notes.remote_has_text {
            outgoing.push(channel.initiate_paste(ClipboardFormatId::CF_UNICODETEXT)?);
        }
        if notes.text_requested {
            let ours = self.local_text.clone().unwrap_or_default();
            let reply = FormatDataResponse::new_unicode_string(&ours).into_owned();
            outgoing.push(channel.submit_format_data(reply)?);
        }
        if notes.other_requested {
            let refusal = FormatDataResponse::new_error().into_owned();
            outgoing.push(channel.submit_format_data(refusal)?);
        }
        for messages in outgoing {
            let bytes = self.stage.process_svc_processor_messages(messages)?;
            self.framed.write_all(&bytes)?;
        }
        for copied in notes.received {
            // A full queue only delays the text.
            let _ = events.try_send(Event::Clipboard(copied));
        }
        Ok(())
    }

    /// Passes the picture on when it changed. False when the window is gone.
    fn send_frame(&mut self, events: &mpsc::Sender<Event>) -> bool {
        if !self.dirty || self.last_sent.elapsed() < FRAME_INTERVAL {
            return true;
        }
        // A full queue means the window is behind; this frame is skipped and a later one follows.
        match events.try_send(Event::Frame(copy_frame(&self.image))) {
            Ok(()) => {
                self.dirty = false;
                self.last_sent = Instant::now();
                self.counter.frame_sent();
                true
            }
            Err(mpsc::error::TrySendError::Full(_)) => true,
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }
}

/// Windows ignores a size request that comes while the desktop is still
/// being set up, so it is held back for a moment.
fn resize_allowed(since_start: Duration) -> bool {
    since_start >= RESIZE_AFTER
}

fn new_image(width: u16, height: u16) -> DecodedImage {
    DecodedImage::new(PixelFormat::RgbA32, width, height)
}

fn is_timeout(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

fn copy_frame(image: &DecodedImage) -> Frame {
    let mut rgba = image.data().to_vec();
    // The decoder leaves alpha undefined; the window needs opaque pixels.
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    Frame {
        width: u32::from(image.width()),
        height: u32::from(image.height()),
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_request_waits_for_the_desktop_to_settle() {
        assert!(!resize_allowed(Duration::from_millis(500)));
        assert!(resize_allowed(RESIZE_AFTER));
    }
}
