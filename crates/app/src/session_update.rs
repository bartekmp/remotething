//! What each message does to an SSH session, and how a live terminal sends
//! input and pastes.

use std::cell::Cell;

use iced::Task;
use iced::widget::canvas::Cache;
use remotething_remote::ssh::{Connection, Details, ShellEvent, ShellInput};
use remotething_secrets::Secret;
use tokio::sync::mpsc;

use crate::challenge;
use crate::console_recording::Recording;
use crate::credentials::Hidden;
use crate::features;
use crate::files_pane;
use crate::forwards;
use crate::i18n::fl;
use crate::keys;
use crate::meter::{Meter, Totals};
use crate::note::Note;
use crate::paste;
use crate::screenshot;
use crate::search;
use crate::session::{
    Context, Failure, INITIAL_SIZE, Message, Method, Sealed, SealedJumps, Stage, State,
};
use crate::terminal::Screen;
use crate::terminal_mouse;
use crate::terminal_view;

pub(crate) struct Live {
    pub(crate) screen: Screen,
    pub(crate) cache: Cache,
    pub(crate) input: mpsc::Sender<ShellInput>,
    /// A paste waiting for a yes or no, because it is long or has line breaks.
    pub(crate) pending_paste: Option<String>,
    /// The connection the shell runs on, for the file pane. `None` only in tests.
    pub(crate) connection: Option<Connection>,
    /// The file pane, while it is open.
    pub(crate) files: Option<files_pane::State>,
    /// The port forwards of this session.
    pub(crate) forwards: forwards::Active,
    /// The search bar, while it is open.
    pub(crate) search: Option<search::Search>,
    /// What is known about the connection, for the details panel.
    pub(crate) details: Details,
    pub(crate) details_open: bool,
    pub(crate) meter: Meter,
    /// The recording of this terminal, while there is one.
    pub(crate) recording: Option<Recording>,
    /// Why recording stopped by itself, or could not start.
    pub(crate) recording_problem: Option<String>,
    /// How the last screenshot went, for a few seconds.
    pub(crate) note: Option<Note>,
    /// The name recordings and screenshots of this terminal are given.
    name: String,
    /// Bytes of shell output received; input sent is counted in [`Live::send`].
    received: u64,
    sent: Cell<u64>,
}

impl Live {
    pub(crate) fn new(
        input: mpsc::Sender<ShellInput>,
        connection: Option<Connection>,
        name: &str,
    ) -> Self {
        let mut live = Self {
            screen: Screen::new(
                usize::from(INITIAL_SIZE.columns),
                usize::from(INITIAL_SIZE.lines),
            ),
            cache: Cache::new(),
            input,
            pending_paste: None,
            files: None,
            forwards: forwards::Active::default(),
            search: None,
            details: connection
                .as_ref()
                .map(Connection::details)
                .unwrap_or_default(),
            details_open: false,
            meter: Meter::new(),
            recording: None,
            recording_problem: None,
            note: None,
            name: name.to_string(),
            received: 0,
            sent: Cell::new(0),
            connection,
        };
        if features::record_every_session() {
            live.start_recording();
        }
        live
    }

    /// Starts recording this terminal, if the feature is on and it is not recording yet.
    pub(crate) fn start_recording(&mut self) {
        if !features::console_recording() || self.recording.is_some() {
            return;
        }
        self.recording_problem = None;
        let (columns, lines) = (self.screen.columns(), self.screen.lines());
        let started = Recording::start(
            &features::console_recordings_folder(),
            &self.name,
            u16::try_from(columns).unwrap_or(u16::MAX),
            u16::try_from(lines).unwrap_or(u16::MAX),
            features::record_keystrokes(),
        );
        match started {
            Ok(recording) => self.recording = Some(recording),
            Err(error) => {
                self.recording_problem = Some(format!("{} {error}", fl!("recording-failed")));
            }
        }
    }

    /// Ends the recording. The file is complete as it is.
    pub(crate) fn stop_recording(&mut self) {
        self.recording = None;
    }

    /// Writes to the recording; a write that fails ends it and says why.
    fn record(&mut self, write: impl FnOnce(&mut Recording) -> std::io::Result<()>) {
        let Some(recording) = &mut self.recording else {
            return;
        };
        if let Err(error) = write(recording) {
            self.recording = None;
            self.recording_problem = Some(format!("{} {error}", fl!("recording-stopped")));
        }
    }

    /// What the user typed or pasted, on its way to the shell.
    pub(crate) fn type_in(&mut self, bytes: Vec<u8>) {
        self.record(|recording| recording.input(&bytes));
        self.send(ShellInput::Data(bytes));
    }

    /// Takes a reading of the traffic so far.
    fn sample(&mut self) {
        self.meter.sample(Totals {
            received: self.received,
            sent: self.sent.get(),
            frames: 0,
        });
    }

    /// Queues input for the shell. When the queue is full the input is dropped
    /// rather than freezing the window.
    pub(crate) fn send(&self, input: ShellInput) {
        if let ShellInput::Data(bytes) = &input {
            self.sent.set(self.sent.get() + bytes.len() as u64);
        }
        let _ = self.input.try_send(input);
    }

    /// Pastes text, after a question when it is long or has line breaks.
    pub(crate) fn paste(&mut self, text: String) {
        if paste::needs_confirmation(&text) {
            self.pending_paste = Some(text);
        } else {
            self.send_paste(&text);
        }
    }

    pub(crate) fn confirm_paste(&mut self) {
        if let Some(text) = self.pending_paste.take() {
            self.send_paste(&text);
        }
    }

    pub(crate) fn send_paste(&mut self, text: &str) {
        let bytes = paste::to_bytes(text, self.screen.bracketed_paste());
        if self.screen.scrolled_back() > 0 {
            self.screen.scroll_to_bottom();
            self.cache.clear();
        }
        self.type_in(bytes);
    }
    /// Something the terminal widget reports: the mouse, a resize, scrolling or a selection.
    fn on_terminal_event(&mut self, event: terminal_view::Event) -> Task<Message> {
        if let terminal_view::Event::Mouse(report) = event {
            if let Some(bytes) = terminal_mouse::encode(&report, self.screen.mouse_mode()) {
                self.type_in(bytes);
            }
            return Task::none();
        }
        self.cache.clear();
        match event {
            terminal_view::Event::Mouse(_) => {}
            terminal_view::Event::Resized(size) => {
                self.screen
                    .resize(usize::from(size.columns), usize::from(size.lines));
                self.record(|recording| recording.resize(size.columns, size.lines));
                self.send(ShellInput::Resize(size));
            }
            terminal_view::Event::Scrolled(lines) if self.screen.wheel_sends_arrows() => {
                let application = self.screen.application_cursor();
                self.type_in(keys::wheel_arrows(lines, application));
            }
            terminal_view::Event::Scrolled(lines) => self.screen.scroll(lines),
            // The window changes the text size; see `State::zoom_terminal`.
            terminal_view::Event::Zoomed(_) => {}
            terminal_view::Event::SelectionStarted {
                column,
                line,
                granularity,
            } => {
                self.screen.start_selection_by(column, line, granularity);
            }
            terminal_view::Event::SelectionExtended { column, line } => {
                self.screen.extend_selection(column, line);
            }
            // Copy on select: the text goes to the selection that middle-click pastes.
            terminal_view::Event::PasteRequested => {
                return iced::clipboard::read_primary().map(Message::Pasted);
            }
            terminal_view::Event::SelectionEnded => {
                if let Some(text) = self.screen.selected_text() {
                    return iced::clipboard::write_primary(text);
                }
            }
        }
        Task::none()
    }

    /// Bytes the shell printed: draw them, answer what the terminal must answer, and
    /// pass on what a program copied to the clipboard.
    fn on_output(&mut self, bytes: &[u8]) -> Task<Message> {
        self.received += bytes.len() as u64;
        self.record(|recording| recording.output(bytes));
        let replies = self.screen.feed(bytes);
        // New output moves the lines under the highlights.
        self.screen.clear_found();
        self.cache.clear();
        if !replies.is_empty() {
            self.send(ShellInput::Data(replies));
        }
        // A program copied text itself: tmux and herdr do, as the mouse is theirs.
        let copies: Vec<_> = self
            .screen
            .take_copied()
            .into_iter()
            .map(|copied| {
                if copied.primary {
                    iced::clipboard::write_primary(copied.text)
                } else {
                    iced::clipboard::write(copied.text)
                }
            })
            .collect();
        Task::batch(copies)
    }
}

impl State {
    /// The sign-in form was sent: check the user name, keep what was typed, go on.
    fn on_submitted(&mut self, context: &mut Context<'_>) -> Task<Message> {
        let Stage::Asking(form) = &mut self.stage else {
            return Task::none();
        };
        let username = form.username.trim().to_string();
        if username.is_empty() {
            form.note = Some(fl!("session-username-missing"));
            return Task::none();
        }
        let secret = Secret::new(std::mem::take(&mut form.secret));
        if let Some(index) = self.asking_jump {
            self.jumps[index].set_typed(username, secret);
            return self.ask_for_jumps(context);
        }
        self.username = username;
        self.set_secret(Some(secret));
        self.connect(context)
    }

    /// The server refused the credentials. What can be done depends on the method.
    fn on_refused(&mut self) -> Task<Message> {
        match self.method {
            Method::Password(_) => {
                self.set_secret(None);
                self.ask(Some(fl!("session-refused")))
            }
            // Typing something would not help: the server does not accept this key.
            Method::KeyFile { .. } => {
                self.stage = Stage::Ended(fl!("session-key-refused"));
                Task::none()
            }
            Method::Agent => {
                self.stage = Stage::Ended(fl!("session-agent-refused"));
                Task::none()
            }
            Method::Interactive => {
                self.stage = Stage::Ended(fl!("session-interactive-refused"));
                Task::none()
            }
        }
    }

    /// The user trusts the key the server showed: remember it and go on.
    fn on_trust(&mut self, context: &mut Context<'_>) -> Task<Message> {
        let Stage::HostKey { key, .. } = &self.stage else {
            return Task::none();
        };
        // The key may belong to a jump host rather than to the target.
        let (host, port) = (&key.endpoint.host, key.endpoint.port);
        match context.known_hosts.trust(host, port, &key.key) {
            Ok(()) => self.connect(context),
            Err(error) => {
                let reason = format!("{} {error}", fl!("session-trust-failed"));
                self.stage = Stage::Ended(reason);
                Task::none()
            }
        }
    }
}

pub fn update(state: &mut State, message: Message, context: &mut Context<'_>) -> Task<Message> {
    match message {
        Message::JumpsLoaded(Ok(SealedJumps(jumps))) => {
            state.jumps = jumps;
            state.ask_for_jumps(context)
        }
        Message::JumpsLoaded(Err(reason)) => {
            state.stage = Stage::Ended(reason);
            Task::none()
        }
        Message::SecretLoaded(Ok(Sealed(Some(secret)))) => {
            state.set_secret(Some(secret));
            state.connect(context)
        }
        // Nothing stored: a key file may not need a passphrase, a password must be typed.
        Message::SecretLoaded(Ok(Sealed(None))) => {
            if matches!(state.method, Method::Password(None)) {
                state.ask(Some(fl!("session-secret-missing")))
            } else {
                state.connect(context)
            }
        }
        Message::SecretLoaded(Err(reason)) => {
            let note = format!("{} {reason}", fl!("session-secret-failed"));
            state.ask(Some(note))
        }
        Message::UsernameChanged(username) => {
            if let Stage::Asking(form) = &mut state.stage {
                form.username = username;
            }
            Task::none()
        }
        Message::SecretChanged(Hidden(secret)) => {
            if let Stage::Asking(form) = &mut state.stage {
                form.secret = secret;
            }
            Task::none()
        }
        Message::Submitted => state.on_submitted(context),
        Message::Opened(Ok(opened)) => state.go_live(opened),
        Message::Opened(Err(Failure::HostKey { key, changed })) => {
            state.stage = Stage::HostKey { key, changed };
            Task::none()
        }
        Message::Opened(Err(Failure::KeyLocked)) => state.ask(Some(fl!("session-key-locked"))),
        Message::Opened(Err(Failure::Refused)) => state.on_refused(),
        Message::Opened(Err(Failure::Other(reason))) => {
            state.stage = Stage::Ended(reason);
            Task::none()
        }
        Message::TrustPressed => state.on_trust(context),
        Message::ReconnectPressed => state.begin(context),
        Message::Shell(ShellEvent::Output(bytes)) => match &mut state.stage {
            Stage::Connected(live) => live.on_output(&bytes),
            _ => Task::none(),
        },
        Message::Tick => {
            let Stage::Connected(live) = &mut state.stage else {
                // The session ended: no more readings.
                return Task::none();
            };
            live.sample();
            next_tick()
        }
        Message::ScreenshotPressed => {
            let Stage::Connected(live) = &state.stage else {
                return Task::none();
            };
            Task::perform(
                screenshot::save_text(
                    features::screenshots_folder(),
                    live.name.clone(),
                    live.screen.text(),
                ),
                Message::ScreenshotSaved,
            )
        }
        Message::ScreenshotSaved(result) => {
            if let Stage::Connected(live) = &mut state.stage {
                live.note = Some(screenshot::note_for(result));
            }
            Task::none()
        }
        Message::RecordPressed => {
            if let Stage::Connected(live) = &mut state.stage {
                if live.recording.is_some() {
                    live.stop_recording();
                } else {
                    live.start_recording();
                }
            }
            Task::none()
        }
        Message::DetailsToggled => {
            if let Stage::Connected(live) = &mut state.stage {
                live.details_open = !live.details_open;
            }
            Task::none()
        }
        Message::Challenged(asked) => {
            if let (Stage::Connecting, Some(challenge)) = (&state.stage, asked.take()) {
                state.stage = Stage::Questions(challenge::Form::new(challenge));
            }
            Task::none()
        }
        Message::AnswerChanged(index, Hidden(text)) => {
            if let Stage::Questions(form) = &mut state.stage {
                form.set_answer(index, text);
            }
            Task::none()
        }
        Message::AnswersSubmitted => {
            if let Stage::Questions(form) = std::mem::replace(&mut state.stage, Stage::Connecting) {
                form.submit();
            }
            Task::none()
        }
        // Dropping the form cancels the sign-in, which ends the attempt.
        Message::AnswersCancelled => {
            state.stage = Stage::Connecting;
            Task::none()
        }
        Message::ForwardsStarted(started) => {
            if let (Stage::Connected(live), Some(active)) = (&mut state.stage, started.take()) {
                live.forwards = active;
            }
            Task::none()
        }
        Message::Shell(ShellEvent::Closed(status)) => {
            state.stage = Stage::Ended(match status {
                Some(status) => fl!("session-closed-status", status = status.to_string()),
                None => fl!("session-closed"),
            });
            Task::none()
        }
        Message::Terminal(event) => match &mut state.stage {
            Stage::Connected(live) => live.on_terminal_event(event),
            _ => Task::none(),
        },
        Message::Pasted(text) => {
            if let (Stage::Connected(live), Some(text)) = (&mut state.stage, text)
                && !text.is_empty()
            {
                live.paste(text);
            }
            Task::none()
        }
        Message::BrowseFilesPressed => {
            let Stage::Connected(live) = &mut state.stage else {
                return Task::none();
            };
            let Some(connection) = live.connection.clone() else {
                return Task::none();
            };
            let (mut pane, task) = files_pane::State::open(connection);
            // Beside the terminal the pane is narrow: names first, details when zoomed.
            pane.details = false;
            live.files = Some(pane);
            task.map(Message::Files)
        }
        Message::Files(message) => {
            if let Stage::Browsing(browser) = &mut state.stage {
                return files_pane::update(&mut browser.files, message)
                    .0
                    .map(Message::Files);
            }
            let Stage::Connected(live) = &mut state.stage else {
                return Task::none();
            };
            let Some(pane) = &mut live.files else {
                return Task::none();
            };
            let (task, event) = files_pane::update(pane, message);
            if event == Some(files_pane::Event::Close) {
                live.files = None;
            }
            task.map(Message::Files)
        }
        Message::SearchPressed => {
            let Stage::Connected(live) = &mut state.stage else {
                return Task::none();
            };
            if live.search.is_some() {
                live.close_search();
                return Task::none();
            }
            live.open_search();
            iced::widget::operation::focus(search::INPUT)
        }
        Message::SearchChanged(query) => {
            if let Stage::Connected(live) = &mut state.stage {
                live.search_for(query);
            }
            Task::none()
        }
        Message::SearchNext | Message::SearchPrevious => {
            if let Stage::Connected(live) = &mut state.stage {
                live.search_step(matches!(message, Message::SearchNext));
            }
            Task::none()
        }
        Message::SearchClosed => {
            if let Stage::Connected(live) = &mut state.stage {
                live.close_search();
            }
            Task::none()
        }
        Message::PasteConfirmed => {
            if let Stage::Connected(live) = &mut state.stage {
                live.confirm_paste();
            }
            Task::none()
        }
        Message::PasteCancelled => {
            if let Stage::Connected(live) = &mut state.stage {
                live.pending_paste = None;
            }
            Task::none()
        }
    }
}

/// How often the traffic is read.
const TICK: std::time::Duration = std::time::Duration::from_secs(1);

/// Asks for the next reading of the traffic after a second.
pub(crate) fn next_tick() -> Task<Message> {
    Task::perform(async { tokio::time::sleep(TICK).await }, |()| Message::Tick)
}
