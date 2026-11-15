//! A Windows Remote Desktop tab: signing in, the certificate question and the
//! live picture with the user's mouse and keyboard going back.

use std::sync::{Arc, Mutex};

use iced::Task;
use iced::futures::stream;
use iced::widget::image::Handle;
use remotething_core::{CredentialKind, ResolvedConnection};
use remotething_rdp::{Button, Certificate, Error, Event, Input, Session, Target};
use remotething_secrets::Secret;
use tokio::sync::mpsc;

use crate::credentials::Hidden;
use crate::i18n::fl;
use crate::scancodes::Scancode;
use crate::session::{Context, Sealed};

/// The size of the remote desktop until resizing is supported.
const WIDTH: u16 = 1280;
const HEIGHT: u16 = 800;

pub struct State {
    pub(crate) connection: ResolvedConnection,
    /// The user typed in a connection string, for a connection without a credential.
    typed_username: Option<String>,
    username: String,
    domain: Option<String>,
    password: Option<Secret>,
    pub(crate) stage: Stage,
}

pub(crate) enum Stage {
    /// Reading the stored password.
    LoadingSecret,
    /// Waiting for the user to type what is missing.
    Asking(Form),
    Connecting,
    /// The server's certificate is new or has changed; the user decides.
    Certificate {
        certificate: Certificate,
        changed: bool,
    },
    Connected(Box<Live>),
    /// Closed or failed, with the reason shown.
    Ended(String),
}

pub(crate) struct Form {
    pub(crate) ask_username: bool,
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) note: Option<String>,
}

/// A running desktop.
pub(crate) struct Live {
    pub(crate) frame: Handle,
    /// The size of the remote desktop in pixels.
    pub(crate) size: (u32, u32),
    input: mpsc::Sender<Input>,
    /// Where the pointer is on the remote desktop, for button presses.
    pointer: (u16, u16),
}

/// A freshly opened session on its way to the window. It is taken out once.
#[derive(Clone)]
pub struct Opened(Arc<Mutex<Option<Session>>>);

impl std::fmt::Debug for Opened {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Opened")
    }
}

/// Why opening the session did not work, in the shape the window acts on.
#[derive(Debug, Clone)]
pub enum Failure {
    Certificate {
        certificate: Certificate,
        changed: bool,
    },
    Refused,
    Other(String),
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        match error {
            Error::UnknownCertificate(certificate) => Failure::Certificate {
                certificate,
                changed: false,
            },
            Error::CertificateChanged(certificate) => Failure::Certificate {
                certificate,
                changed: true,
            },
            Error::AuthRefused => Failure::Refused,
            other => Failure::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    SecretLoaded(Result<Sealed, String>),
    UsernameChanged(String),
    PasswordChanged(Hidden),
    Submitted,
    Opened(Result<Opened, Failure>),
    /// The desktop changed: a new picture and its size.
    Frame(Handle, u32, u32),
    /// The session ended, with the reason when it was not the user's doing.
    Closed(Option<String>),
    TrustPressed,
    ReconnectPressed,
    /// The pointer moved over the picture, in pixels of the remote desktop.
    Moved(u16, u16),
    /// A mouse button went down (true) or up.
    Button(Button, bool),
    /// The wheel turned; positive is up, 120 per notch.
    Wheel(i16),
}

impl State {
    /// Works out how to sign in and starts as far as it can without the user.
    pub fn start(
        connection: ResolvedConnection,
        typed_username: Option<String>,
        context: &Context<'_>,
    ) -> (Self, Task<Message>) {
        let mut state = State {
            connection,
            typed_username,
            username: String::new(),
            domain: None,
            password: None,
            stage: Stage::Connecting,
        };
        let task = state.begin(context);
        (state, task)
    }

    /// Puts the tab into the connected stage without a server.
    #[cfg(test)]
    pub(crate) fn connect_for_test(&mut self) -> mpsc::Receiver<Input> {
        let (input, typed) = mpsc::channel(16);
        self.stage = Stage::Connected(Box::new(Live {
            frame: Handle::from_rgba(1, 1, vec![0, 0, 0, 255]),
            size: (1280, 800),
            input,
            pointer: (0, 0),
        }));
        typed
    }

    /// Whether a desktop is showing, so closing the tab would cut it.
    pub(crate) fn is_connected(&self) -> bool {
        matches!(self.stage, Stage::Connected(_))
    }

    fn begin(&mut self, context: &Context<'_>) -> Task<Message> {
        let credential = self.connection.credential.as_ref().and_then(|id| {
            context
                .store
                .credentials
                .iter()
                .find(|credential| &credential.id == id)
        });
        let Some(credential) = credential else {
            self.username = self.typed_username.clone().unwrap_or_default();
            self.password = None;
            return self.ask(None);
        };
        self.username = credential.username.clone().unwrap_or_default();
        self.domain = credential
            .domain
            .clone()
            .filter(|domain| !domain.is_empty());
        match credential.kind {
            CredentialKind::Password => {
                self.stage = Stage::LoadingSecret;
                let secrets = Arc::clone(context.secrets);
                let id = credential.id.clone();
                // The keyring may ask to be unlocked, so this stays off the UI thread.
                Task::perform(
                    async move {
                        secrets
                            .get(&id)
                            .map(Sealed)
                            .map_err(|error| error.to_string())
                    },
                    Message::SecretLoaded,
                )
            }
            CredentialKind::Ask => self.ask(None),
            // Keys, the agent and keyboard-interactive are SSH ways of signing in.
            _ => {
                self.stage = Stage::Ended(fl!("desktop-credential-kind"));
                Task::none()
            }
        }
    }

    fn ask(&mut self, note: Option<String>) -> Task<Message> {
        self.stage = Stage::Asking(Form {
            ask_username: self.username.is_empty(),
            username: self.username.clone(),
            password: String::new(),
            note,
        });
        Task::none()
    }

    /// Connects with what is known, or asks for what is missing.
    fn connect(&mut self, context: &Context<'_>) -> Task<Message> {
        let Some(password) = self.password.clone().filter(|_| !self.username.is_empty()) else {
            return self.ask(None);
        };
        let target = Target {
            host: self.connection.host.clone(),
            port: self.connection.port,
            username: self.username.clone(),
            password,
            domain: self.domain.clone(),
            width: WIDTH,
            height: HEIGHT,
        };
        let known = context.known_hosts.clone();
        self.stage = Stage::Connecting;
        Task::perform(
            async move {
                // Connecting blocks until the desktop is up.
                let opened =
                    tokio::task::spawn_blocking(move || remotething_rdp::connect(target, &known))
                        .await;
                match opened {
                    Ok(Ok(session)) => Ok(Opened(Arc::new(Mutex::new(Some(session))))),
                    Ok(Err(error)) => Err(Failure::from(error)),
                    Err(error) => Err(Failure::Other(error.to_string())),
                }
            },
            Message::Opened,
        )
    }

    fn go_live(&mut self, opened: Opened) -> Task<Message> {
        let session = opened.0.lock().ok().and_then(|mut session| session.take());
        let Some(session) = session else {
            return Task::none();
        };
        let Session { events, input } = session;
        // Black until the first picture arrives.
        let blank = Handle::from_rgba(1, 1, vec![0, 0, 0, 255]);
        self.stage = Stage::Connected(Box::new(Live {
            frame: blank,
            size: (u32::from(WIDTH), u32::from(HEIGHT)),
            input,
            pointer: (0, 0),
        }));

        // The stream ends when the session thread stops, for example once this tab is closed.
        let messages = stream::unfold(events, |mut events| async move {
            let message = match events.recv().await? {
                Event::Frame(frame) => Message::Frame(
                    Handle::from_rgba(frame.width, frame.height, frame.rgba),
                    frame.width,
                    frame.height,
                ),
                Event::Closed(reason) => Message::Closed(reason),
            };
            Some((message, events))
        });
        Task::run(messages, |message| message)
    }

    /// A key went down or up while this desktop was in front.
    pub(crate) fn key(&mut self, scancode: Scancode, pressed: bool) {
        self.send(Input::Key {
            code: scancode.code,
            extended: scancode.extended,
            pressed,
        });
    }

    fn send(&mut self, input: Input) {
        if let Stage::Connected(live) = &mut self.stage {
            // A full queue drops the input; stale mouse moves are not worth waiting for.
            let _ = live.input.try_send(input);
        }
    }
}

pub fn update(state: &mut State, message: Message, context: &mut Context<'_>) -> Task<Message> {
    match message {
        Message::SecretLoaded(Ok(Sealed(Some(secret)))) => {
            state.password = Some(secret);
            state.connect(context)
        }
        // A password credential with nothing stored: it has to be typed.
        Message::SecretLoaded(Ok(Sealed(None))) => state.ask(None),
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
        Message::PasswordChanged(Hidden(password)) => {
            if let Stage::Asking(form) = &mut state.stage {
                form.password = password;
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
            let password = Secret::new(std::mem::take(&mut form.password));
            state.username = username;
            state.password = Some(password);
            state.connect(context)
        }
        Message::Opened(Ok(opened)) => state.go_live(opened),
        Message::Opened(Err(Failure::Certificate {
            certificate,
            changed,
        })) => {
            state.stage = Stage::Certificate {
                certificate,
                changed,
            };
            Task::none()
        }
        Message::Opened(Err(Failure::Refused)) => {
            state.password = None;
            state.ask(Some(fl!("session-refused")))
        }
        Message::Opened(Err(Failure::Other(reason))) => {
            state.stage = Stage::Ended(reason);
            Task::none()
        }
        Message::Frame(frame, width, height) => {
            if let Stage::Connected(live) = &mut state.stage {
                live.frame = frame;
                live.size = (width, height);
            }
            Task::none()
        }
        Message::Closed(reason) => {
            state.stage = Stage::Ended(reason.unwrap_or_else(|| fl!("session-closed")));
            Task::none()
        }
        Message::TrustPressed => {
            let Stage::Certificate { certificate, .. } = &state.stage else {
                return Task::none();
            };
            match context
                .known_hosts
                .trust(&certificate.host, certificate.port, &certificate.key)
            {
                Ok(()) => state.connect(context),
                Err(error) => {
                    let reason = format!("{} {error}", fl!("session-trust-failed"));
                    state.stage = Stage::Ended(reason);
                    Task::none()
                }
            }
        }
        Message::Moved(x, y) => {
            if let Stage::Connected(live) = &mut state.stage {
                live.pointer = (x, y);
            }
            state.send(Input::MouseMoved { x, y });
            Task::none()
        }
        Message::Button(button, pressed) => {
            let (x, y) = state.pointer();
            state.send(Input::Button {
                button,
                pressed,
                x,
                y,
            });
            Task::none()
        }
        Message::Wheel(delta) => {
            let (x, y) = state.pointer();
            state.send(Input::Wheel { delta, x, y });
            Task::none()
        }
        Message::ReconnectPressed => state.begin(context),
    }
}

impl State {
    /// The pointer position on the remote desktop, for presses that carry none.
    pub(crate) fn pointer(&self) -> (u16, u16) {
        match &self.stage {
            Stage::Connected(live) => live.pointer,
            _ => (0, 0),
        }
    }
}

#[cfg(test)]
#[path = "desktop_tests.rs"]
mod tests;
