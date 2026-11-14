//! What each message does to an SSH session, and how a live terminal sends
//! input and pastes.

use iced::Task;
use iced::widget::canvas::Cache;
use remotething_remote::ssh::{Connection, ShellEvent, ShellInput};
use remotething_secrets::Secret;
use tokio::sync::mpsc;

use crate::credentials::Hidden;
use crate::files_pane;
use crate::i18n::fl;
use crate::paste;
use crate::session::{
    Context, Failure, INITIAL_SIZE, Message, Method, Sealed, SealedJumps, Stage, State,
};
use crate::terminal::Screen;
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
}

impl Live {
    pub(crate) fn new(input: mpsc::Sender<ShellInput>, connection: Option<Connection>) -> Self {
        Self {
            screen: Screen::new(
                usize::from(INITIAL_SIZE.columns),
                usize::from(INITIAL_SIZE.lines),
            ),
            cache: Cache::new(),
            input,
            pending_paste: None,
            connection,
            files: None,
        }
    }

    /// Queues input for the shell. When the queue is full the input is dropped
    /// rather than freezing the window.
    pub(crate) fn send(&self, input: ShellInput) {
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
        self.send(ShellInput::Data(bytes));
    }
}

pub fn update(state: &mut State, message: Message, context: &mut Context<'_>) -> Task<Message> {
    match message {
        Message::JumpsLoaded(Ok(SealedJumps(jumps))) => {
            state.jumps = jumps;
            state.begin_target(context)
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
        Message::SecretLoaded(Ok(Sealed(None))) => state.connect(context),
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
        Message::Submitted => {
            let Stage::Asking(form) = &mut state.stage else {
                return Task::none();
            };
            let username = form.username.trim().to_string();
            if username.is_empty() {
                form.note = Some(fl!("session-username-missing"));
                return Task::none();
            }
            let secret = Secret::new(std::mem::take(&mut form.secret));
            state.username = username;
            state.set_secret(Some(secret));
            state.connect(context)
        }
        Message::Opened(Ok(opened)) => state.go_live(opened),
        Message::Opened(Err(Failure::HostKey { key, changed })) => {
            state.stage = Stage::HostKey { key, changed };
            Task::none()
        }
        Message::Opened(Err(Failure::KeyLocked)) => state.ask(Some(fl!("session-key-locked"))),
        Message::Opened(Err(Failure::Refused)) => match state.method {
            Method::Password(_) => {
                state.set_secret(None);
                state.ask(Some(fl!("session-refused")))
            }
            // Typing something would not help: the server does not accept this key.
            Method::KeyFile { .. } => {
                state.stage = Stage::Ended(fl!("session-key-refused"));
                Task::none()
            }
        },
        Message::Opened(Err(Failure::Other(reason))) => {
            state.stage = Stage::Ended(reason);
            Task::none()
        }
        Message::TrustPressed => {
            let Stage::HostKey { key, .. } = &state.stage else {
                return Task::none();
            };
            // The key may belong to a jump host rather than to the target.
            let (host, port) = (&key.endpoint.host, key.endpoint.port);
            match context.known_hosts.trust(host, port, &key.key) {
                Ok(()) => state.connect(context),
                Err(error) => {
                    let reason = format!("{} {error}", fl!("session-trust-failed"));
                    state.stage = Stage::Ended(reason);
                    Task::none()
                }
            }
        }
        Message::ReconnectPressed => state.begin(context),
        Message::Shell(ShellEvent::Output(bytes)) => {
            if let Stage::Connected(live) = &mut state.stage {
                let replies = live.screen.feed(&bytes);
                live.cache.clear();
                if !replies.is_empty() {
                    live.send(ShellInput::Data(replies));
                }
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
        Message::Terminal(event) => {
            let Stage::Connected(live) = &mut state.stage else {
                return Task::none();
            };
            live.cache.clear();
            match event {
                terminal_view::Event::Resized(size) => {
                    live.screen
                        .resize(usize::from(size.columns), usize::from(size.lines));
                    live.send(ShellInput::Resize(size));
                }
                terminal_view::Event::Scrolled(lines) => live.screen.scroll(lines),
                terminal_view::Event::SelectionStarted { column, line } => {
                    live.screen.start_selection(column, line);
                }
                terminal_view::Event::SelectionExtended { column, line } => {
                    live.screen.extend_selection(column, line);
                }
                // Copy on select: the text goes to the selection that middle-click pastes.
                terminal_view::Event::PasteRequested => {
                    return iced::clipboard::read_primary().map(Message::Pasted);
                }
                terminal_view::Event::SelectionEnded => {
                    if let Some(text) = live.screen.selected_text() {
                        return iced::clipboard::write_primary(text);
                    }
                }
            }
            Task::none()
        }
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
            let (pane, task) = files_pane::State::open(connection);
            live.files = Some(pane);
            task.map(Message::Files)
        }
        Message::Files(message) => {
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
