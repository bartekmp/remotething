//! One SSH session in a tab: from asking for what is missing, through the
//! host-key question, to the live terminal and its end.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use iced::Task;
use iced::futures::stream;
use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use remotething_core::{CredentialKind, ResolvedConnection, Store};
#[cfg(test)]
use remotething_remote::ssh::ShellInput;
use remotething_remote::ssh::{
    self, Auth, Endpoint, Hop, HostKey, Shell, ShellEvent, TerminalSize,
};
use remotething_secrets::{KnownHosts, Secret, SecretStore};

use crate::challenge;
use crate::credentials::Hidden;
use crate::files_pane;
use crate::forwards;
use crate::i18n::fl;
use crate::jump;
use crate::keys;
use crate::search;
pub(crate) use crate::session_update::{Live, update};
use crate::terminal_view;

/// The terminal's size until the view reports the real one.
pub(crate) const INITIAL_SIZE: TerminalSize = TerminalSize {
    columns: 100,
    lines: 30,
};

/// What a session needs from the window.
pub struct Context<'a> {
    pub store: &'a Store,
    pub secrets: &'a Arc<dyn SecretStore>,
    pub known_hosts: &'a mut KnownHosts,
}

pub struct State {
    pub(crate) connection: ResolvedConnection,
    /// The user typed in a connection string, for a connection without a credential.
    pub(crate) typed_username: Option<String>,
    /// The jump hosts on the way, in the order they are connected to.
    pub(crate) jumps: Vec<jump::Ready>,
    pub(crate) username: String,
    pub(crate) method: Method,
    pub(crate) stage: Stage,
}

/// How to sign in, with the secret once it is known.
pub(crate) enum Method {
    /// The keys of a running ssh-agent; nothing is typed or stored.
    Agent,
    /// The server asks the questions; nothing is known beforehand.
    Interactive,
    Password(Option<Secret>),
    KeyFile {
        path: PathBuf,
        passphrase: Option<Secret>,
    },
}

pub(crate) enum Stage {
    /// Reading the stored password or passphrase.
    LoadingSecret,
    /// Waiting for the user to type what is missing.
    Asking(Form),
    Connecting,
    /// The server asks questions at sign-in; the user answers.
    Questions(challenge::Form),
    /// The server's key is new or has changed; the user decides.
    HostKey {
        key: HostKey,
        changed: bool,
    },
    Connected(Box<Live>),
    /// Closed or failed, with the reason shown.
    Ended(String),
}

/// The sign-in form of the asking stage.
impl State {
    /// Whether a shell is running, so closing the tab would cut it.
    pub(crate) fn is_connected(&self) -> bool {
        matches!(self.stage, Stage::Connected(_))
    }

    /// Puts the session into the connected stage without a server.
    #[cfg(test)]
    pub(crate) fn connect_for_test(&mut self) -> tokio::sync::mpsc::Receiver<ShellInput> {
        let (input, typed) = tokio::sync::mpsc::channel(4);
        self.stage = Stage::Connected(Box::new(Live::new(input, None, &self.connection.name)));
        typed
    }

    /// Whether the form of the asking stage wants a key passphrase rather than a password.
    pub(crate) fn asks_for_passphrase(&self) -> bool {
        matches!(self.method, Method::KeyFile { .. })
    }
}

pub(crate) struct Form {
    pub(crate) ask_username: bool,
    pub(crate) username: String,
    /// The password or passphrase as typed.
    pub(crate) secret: String,
    /// Why the form is shown again, such as a refused password.
    pub(crate) note: Option<String>,
}

/// A stored secret on its way from the worker thread; never printed.
#[derive(Clone)]
pub struct Sealed(pub(crate) Option<Secret>);

impl std::fmt::Debug for Sealed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Sealed")
    }
}

/// A freshly opened shell on its way to the window. It is taken out once.
#[derive(Clone)]
pub struct Opened(Arc<Mutex<Option<Shell>>>);

impl std::fmt::Debug for Opened {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Opened")
    }
}

/// Why opening the shell did not work, in the shape the window acts on.
#[derive(Debug, Clone)]
pub enum Failure {
    HostKey { key: HostKey, changed: bool },
    KeyLocked,
    Refused,
    Other(String),
}

impl From<ssh::Error> for Failure {
    fn from(error: ssh::Error) -> Self {
        match error {
            ssh::Error::UnknownHostKey(key) => Failure::HostKey {
                key,
                changed: false,
            },
            ssh::Error::HostKeyChanged(key) => Failure::HostKey { key, changed: true },
            ssh::Error::KeyLocked(_) => Failure::KeyLocked,
            ssh::Error::AuthRefused => Failure::Refused,
            other => Failure::Other(other.to_string()),
        }
    }
}

/// Jump hosts with their secrets on the way from the worker thread; never printed.
#[derive(Clone)]
pub struct SealedJumps(pub(crate) Vec<jump::Ready>);

impl std::fmt::Debug for SealedJumps {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SealedJumps")
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    /// The stored secrets of the jump hosts were read, or one is missing.
    JumpsLoaded(Result<SealedJumps, String>),
    SecretLoaded(Result<Sealed, String>),
    /// A second has passed: time to read the traffic.
    Tick,
    /// The (i) button was pressed.
    DetailsToggled,
    /// The Record button was pressed.
    RecordPressed,
    /// The Screenshot button was pressed.
    ScreenshotPressed,
    /// A screenshot was saved, or could not be.
    ScreenshotSaved(Result<String, String>),
    UsernameChanged(String),
    SecretChanged(Hidden),
    Submitted,
    Opened(Result<Opened, Failure>),
    Shell(ShellEvent),
    ForwardsStarted(forwards::Started),
    /// The server asked questions during a keyboard-interactive sign-in.
    Challenged(challenge::Asked),
    AnswerChanged(usize, Hidden),
    AnswersSubmitted,
    AnswersCancelled,
    Terminal(terminal_view::Event),
    TrustPressed,
    ReconnectPressed,
    Pasted(Option<String>),
    /// The "Browse files" button of a connected session.
    BrowseFilesPressed,
    Files(files_pane::Message),
    PasteConfirmed,
    PasteCancelled,
    SearchChanged(String),
    SearchNext,
    SearchPrevious,
    SearchClosed,
}

impl State {
    /// Works out how to sign in and starts as far as it can without the user.
    #[cfg(test)]
    pub fn start(connection: ResolvedConnection, context: &Context<'_>) -> (Self, Task<Message>) {
        Self::start_as(connection, None, context)
    }

    /// Like [`State::start`], signing in as `typed_username` when the connection has no credential.
    pub fn start_as(
        connection: ResolvedConnection,
        typed_username: Option<String>,
        context: &Context<'_>,
    ) -> (Self, Task<Message>) {
        let mut state = State {
            connection,
            typed_username,
            jumps: Vec::new(),
            username: String::new(),
            method: Method::Password(None),
            stage: Stage::Connecting,
        };
        let task = state.begin(context);
        (state, task)
    }

    /// Starts over: works out the jump hosts first, then how to sign in to the target.
    pub(crate) fn begin(&mut self, context: &Context<'_>) -> Task<Message> {
        self.jumps = Vec::new();
        let Some(jump) = &self.connection.jump else {
            return self.begin_target(context);
        };
        let planned = match jump::plan(context.store, jump) {
            Ok(planned) => planned,
            Err(reason) => {
                self.stage = Stage::Ended(reason);
                return Task::none();
            }
        };
        // The stored secrets may sit behind a keyring prompt.
        self.stage = Stage::LoadingSecret;
        let secrets = Arc::clone(context.secrets);
        Task::perform(
            async move { jump::load(planned, secrets).map(SealedJumps) },
            Message::JumpsLoaded,
        )
    }

    /// Works out how to sign in to the target and starts as far as it can without the user.
    pub(crate) fn begin_target(&mut self, context: &Context<'_>) -> Task<Message> {
        let credential = self.connection.credential.as_ref().and_then(|id| {
            context
                .store
                .credentials
                .iter()
                .find(|credential| &credential.id == id)
        });
        let Some(credential) = credential else {
            self.username = self.typed_username.clone().unwrap_or_default();
            self.method = Method::Password(None);
            return self.ask(None);
        };

        self.username = credential.username.clone().unwrap_or_default();
        match (credential.kind, &credential.key_file) {
            (CredentialKind::Ask, _) => {
                self.method = Method::Password(None);
                self.ask(None)
            }
            (CredentialKind::Password, _) => {
                self.method = Method::Password(None);
                self.load_secret(context, credential.id.clone())
            }
            (CredentialKind::PrivateKey, Some(path)) => {
                self.method = Method::KeyFile {
                    path: PathBuf::from(path),
                    passphrase: None,
                };
                self.load_secret(context, credential.id.clone())
            }
            (CredentialKind::PrivateKey, None) => {
                self.stage = Stage::Ended(fl!("session-no-key-file"));
                Task::none()
            }
            (CredentialKind::Agent | CredentialKind::Interactive, _) => {
                if self.username.is_empty() {
                    self.stage = Stage::Ended(fl!("session-agent-needs-user"));
                    return Task::none();
                }
                self.method = if credential.kind == CredentialKind::Agent {
                    Method::Agent
                } else {
                    Method::Interactive
                };
                self.connect(context)
            }
        }
    }

    /// Reads the stored secret off the UI thread; the keyring may ask to be unlocked.
    pub(crate) fn load_secret(
        &mut self,
        context: &Context<'_>,
        credential_id: String,
    ) -> Task<Message> {
        self.stage = Stage::LoadingSecret;
        let secrets = Arc::clone(context.secrets);
        Task::perform(
            async move {
                let found = secrets.get(&credential_id);
                found.map(Sealed).map_err(|error| error.to_string())
            },
            Message::SecretLoaded,
        )
    }

    pub(crate) fn ask(&mut self, note: Option<String>) -> Task<Message> {
        self.stage = Stage::Asking(Form {
            ask_username: self.username.is_empty(),
            username: self.username.clone(),
            secret: String::new(),
            note,
        });
        Task::none()
    }

    pub(crate) fn set_secret(&mut self, secret: Option<Secret>) {
        match &mut self.method {
            Method::Agent | Method::Interactive => {}
            Method::Password(password) => *password = secret,
            Method::KeyFile { passphrase, .. } => *passphrase = secret,
        }
    }

    /// Connects with what is known, or asks for what is still missing.
    pub(crate) fn connect(&mut self, context: &Context<'_>) -> Task<Message> {
        if self.username.is_empty() {
            return self.ask(None);
        }
        let mut questions = None;
        let auth = match &self.method {
            Method::Interactive => {
                let (auth, asked) = challenge::auth(self.username.clone());
                questions = Some(asked);
                auth
            }
            Method::Agent => Auth::Agent {
                username: self.username.clone(),
                socket: None,
            },
            Method::Password(None) => return self.ask(None),
            Method::Password(Some(password)) => Auth::Password {
                username: self.username.clone(),
                password: password.clone(),
            },
            Method::KeyFile { path, passphrase } => Auth::KeyFile {
                username: self.username.clone(),
                path: path.clone(),
                passphrase: passphrase.clone(),
            },
        };
        let endpoint = Endpoint {
            host: self.connection.host.clone(),
            port: self.connection.port,
        };
        let known = context.known_hosts.clone();
        let hops: Vec<Hop> = self.jumps.iter().filter_map(jump::Ready::hop).collect();

        self.stage = Stage::Connecting;
        let open = Task::perform(
            async move {
                let shell =
                    ssh::open_shell_through(&hops, &endpoint, &auth, known, INITIAL_SIZE).await;
                match shell {
                    Ok(shell) => Ok(Opened(Arc::new(Mutex::new(Some(shell))))),
                    Err(error) => Err(Failure::from(error)),
                }
            },
            Message::Opened,
        );
        match questions {
            Some(asked) => Task::batch([open, challenge::questions(asked)]),
            None => open,
        }
    }

    pub(crate) fn go_live(&mut self, opened: Opened) -> Task<Message> {
        let shell = opened.0.lock().ok().and_then(|mut shell| shell.take());
        let Some(shell) = shell else {
            return Task::none();
        };
        let connection = shell.connection.clone();
        let live = Live::new(shell.input, Some(shell.connection), &self.connection.name);
        self.stage = Stage::Connected(Box::new(live));

        // The stream ends when the session task stops, for example once this tab is closed.
        let events = stream::unfold(shell.events, |mut events| async move {
            let event = events.recv().await?;
            Some((event, events))
        });
        let forwards = forwards::start(connection, self.connection.forwards.clone());
        Task::batch([
            Task::run(events, Message::Shell),
            forwards,
            crate::session_update::next_tick(),
        ])
    }

    /// A key press for the terminal. Ctrl+Shift+V pastes.
    pub fn key(&mut self, key: &Key, modifiers: Modifiers, text: Option<&str>) -> Task<Message> {
        let Stage::Connected(live) = &mut self.stage else {
            return Task::none();
        };
        if live.pending_paste.is_some() {
            match key {
                Key::Named(Named::Enter) => live.confirm_paste(),
                Key::Named(Named::Escape) => live.pending_paste = None,
                // Nothing is typed into the shell while the question is open.
                _ => {}
            }
            return Task::none();
        }
        let is_paste = modifiers.control()
            && modifiers.shift()
            && matches!(key, Key::Character(character) if character.eq_ignore_ascii_case("v"));
        if is_paste {
            return iced::clipboard::read().map(Message::Pasted);
        }
        let is_find = modifiers.control()
            && modifiers.shift()
            && matches!(key, Key::Character(character) if character.eq_ignore_ascii_case("f"));
        if is_find {
            live.open_search();
            return iced::widget::operation::focus(search::INPUT);
        }
        if live.search.is_some() && *key == Key::Named(Named::Escape) {
            live.close_search();
            return Task::none();
        }
        let is_copy = modifiers.control()
            && modifiers.shift()
            && matches!(key, Key::Character(character) if character.eq_ignore_ascii_case("c"));
        if is_copy {
            return match live.screen.selected_text() {
                Some(text) => iced::clipboard::write(text),
                None => Task::none(),
            };
        }

        // Shift with Page Up or Page Down looks through the scrollback instead of typing.
        if modifiers.shift()
            && let Key::Named(named @ (Named::PageUp | Named::PageDown)) = key
        {
            let page = live.screen.lines().saturating_sub(1) as i32;
            live.screen
                .scroll(if *named == Named::PageUp { page } else { -page });
            live.cache.clear();
            return Task::none();
        }

        let application_cursor = live.screen.application_cursor();
        if let Some(bytes) = keys::to_bytes(key, modifiers, text, application_cursor) {
            // Typing brings the newest lines back into view.
            if live.screen.scrolled_back() > 0 {
                live.screen.scroll_to_bottom();
                live.cache.clear();
            }
            live.type_in(bytes);
        }
        Task::none()
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
