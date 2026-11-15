//! A VNC tab: asking for the password, then the live picture with the user's
//! mouse and keyboard going back.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use iced::Task;
use iced::futures::stream;
use iced::keyboard::Key;
use iced::keyboard::key::Physical;
use remotething_core::{CredentialKind, ResolvedConnection};
use remotething_secrets::Secret;
use remotething_vnc::{Button, Error, Event, Info, Input, Session, Stats, Target};
use tokio::sync::mpsc;

use crate::credentials::Hidden;
use crate::desktop_cursor::Look;
use crate::i18n::fl;
use crate::keysym;
use crate::meter::{Meter, Totals};
use crate::picture::Picture;
use crate::session::{Context, Sealed};

pub struct State {
    pub(crate) connection: ResolvedConnection,
    password: Option<Secret>,
    pub(crate) stage: Stage,
}

pub(crate) enum Stage {
    /// Reading the stored password.
    LoadingSecret,
    /// The server wants a password and none is known (or it was refused).
    Asking(Form),
    Connecting,
    Connected(Box<Live>),
    /// Closed or failed, with the reason shown.
    Ended(String),
}

pub(crate) struct Form {
    pub(crate) password: String,
    /// Why the form is shown, such as a refused password.
    pub(crate) note: Option<String>,
}

/// A running desktop.
pub(crate) struct Live {
    pub(crate) picture: Picture,
    /// The size of the remote desktop in pixels.
    pub(crate) size: (u32, u32),
    input: mpsc::Sender<Input>,
    pub(crate) info: Info,
    pub(crate) meter: Meter,
    pub(crate) details_open: bool,
    /// The clipboard text last seen locally or received from the server.
    clipboard: Option<String>,
    /// Where the pointer is on the remote desktop, for button presses.
    pub(crate) pointer: (u16, u16),
    pub(crate) cursor: Look,
    pub(crate) over: bool,
    /// The keysym each held key sent, so its release lets go of the same one.
    held: HashMap<Physical, u32>,
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
    PasswordNeeded,
    WrongPassword,
    Other(String),
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        match error {
            Error::PasswordNeeded => Failure::PasswordNeeded,
            Error::WrongPassword => Failure::WrongPassword,
            other => Failure::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    SecretLoaded(Result<Sealed, String>),
    PasswordChanged(Hidden),
    Submitted,
    Opened(Result<Opened, Failure>),
    ReconnectPressed,
    Frame(Picture),
    Stats(Stats),
    Clipboard(String),
    Cursor(Option<remotething_vnc::Cursor>),
    Closed(Option<String>),
    /// The pointer moved over the picture, in pixels of the remote desktop.
    Moved(u16, u16),
    Button(Button, bool),
    /// The wheel turned; positive is up, 120 per notch.
    Wheel(i16),
    /// The pointer came over the picture or left it.
    PointerOver(bool),
    DetailsToggled,
}

impl State {
    /// Works out how to sign in and starts as far as it can without the user.
    pub fn start(connection: ResolvedConnection, context: &Context<'_>) -> (Self, Task<Message>) {
        let mut state = State {
            connection,
            password: None,
            stage: Stage::Connecting,
        };
        let task = state.begin(context);
        (state, task)
    }

    /// Puts the tab into the connected stage without a server.
    #[cfg(test)]
    pub(crate) fn connect_for_test(&mut self, info: Info) -> mpsc::Receiver<Input> {
        let (input, typed) = mpsc::channel(16);
        self.stage = Stage::Connected(Box::new(Live::new(input, info, (640, 480))));
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
        // Without a credential the server may need no password at all.
        let Some(credential) = credential else {
            self.password = None;
            return self.connect();
        };
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
                self.stage = Stage::Ended(fl!("vnc-credential-kind"));
                Task::none()
            }
        }
    }

    fn ask(&mut self, note: Option<String>) -> Task<Message> {
        self.stage = Stage::Asking(Form {
            password: String::new(),
            note,
        });
        Task::none()
    }

    /// Connects with what is known.
    fn connect(&mut self) -> Task<Message> {
        let target = Target {
            host: self.connection.host.clone(),
            port: self.connection.port,
            password: self.password.clone(),
        };
        self.stage = Stage::Connecting;
        Task::perform(
            async move {
                remotething_vnc::connect(target)
                    .await
                    .map(|session| Opened(Arc::new(Mutex::new(Some(session)))))
                    .map_err(Failure::from)
            },
            Message::Opened,
        )
    }

    fn go_live(&mut self, opened: Opened) -> Task<Message> {
        let session = opened.0.lock().ok().and_then(|mut session| session.take());
        let Some(session) = session else {
            return Task::none();
        };
        let Session {
            info,
            events,
            input,
        } = session;
        // Black until the first picture arrives.
        self.stage = Stage::Connected(Box::new(Live::new(input, info, (1, 1))));

        // The stream ends when the session task stops, for example once this tab is closed.
        let messages = stream::unfold(events, |mut events| async move {
            let message = match events.recv().await? {
                Event::Frame(frame) => {
                    Message::Frame(Picture::new(frame.width, frame.height, frame.rgba))
                }
                Event::Cursor(cursor) => Message::Cursor(cursor),
                Event::Clipboard(text) => Message::Clipboard(text),
                Event::Stats(stats) => Message::Stats(stats),
                Event::Closed(reason) => Message::Closed(reason),
            };
            Some((message, events))
        });
        Task::run(messages, |message| message)
    }

    /// The local clipboard holds `text`. A new text is offered to the server.
    pub(crate) fn local_clipboard(&mut self, text: String) {
        let Stage::Connected(live) = &mut self.stage else {
            return;
        };
        if live.clipboard.as_deref() == Some(text.as_str()) {
            return;
        }
        live.clipboard = Some(text.clone());
        self.send(Input::Clipboard(text));
    }

    /// A key went down or up while this desktop was in front.
    pub(crate) fn key(&mut self, key: &Key, physical: Physical, text: Option<&str>, pressed: bool) {
        let Stage::Connected(live) = &mut self.stage else {
            return;
        };
        let keysym = if pressed {
            let Some(keysym) = keysym::of(key, &physical, text) else {
                return;
            };
            live.held.insert(physical, keysym);
            keysym
        } else {
            // A key held while its meaning changed (Shift let go first) still lets go of what it sent.
            let Some(keysym) = live.held.remove(&physical) else {
                return;
            };
            keysym
        };
        self.send(Input::Key { keysym, pressed });
    }

    fn send(&self, input: Input) {
        if let Stage::Connected(live) = &self.stage {
            // A full queue drops the input; stale mouse moves are not worth waiting for.
            let _ = live.input.try_send(input);
        }
    }
}

impl Live {
    fn new(input: mpsc::Sender<Input>, info: Info, size: (u32, u32)) -> Self {
        Self {
            picture: Picture::black(),
            size,
            input,
            info,
            meter: Meter::new(),
            details_open: false,
            clipboard: None,
            pointer: (0, 0),
            cursor: Look::default(),
            over: false,
            held: HashMap::new(),
        }
    }
}

pub fn update(state: &mut State, message: Message, context: &mut Context<'_>) -> Task<Message> {
    match message {
        Message::SecretLoaded(Ok(Sealed(Some(secret)))) => {
            state.password = Some(secret);
            state.connect()
        }
        // A password credential with nothing stored: it has to be typed.
        Message::SecretLoaded(Ok(Sealed(None))) => state.ask(None),
        Message::SecretLoaded(Err(reason)) => {
            let note = format!("{} {reason}", fl!("session-secret-failed"));
            state.ask(Some(note))
        }
        Message::PasswordChanged(Hidden(password)) => {
            if let Stage::Asking(form) = &mut state.stage {
                form.password = password;
            }
            Task::none()
        }
        Message::Submitted => {
            let Stage::Asking(form) = &state.stage else {
                return Task::none();
            };
            state.password = Some(Secret::new(form.password.clone()));
            state.connect()
        }
        Message::Opened(Ok(opened)) => state.go_live(opened),
        Message::Opened(Err(Failure::PasswordNeeded)) => state.ask(Some(fl!("vnc-needs-password"))),
        Message::Opened(Err(Failure::WrongPassword)) => {
            state.password = None;
            state.ask(Some(fl!("vnc-refused")))
        }
        Message::Opened(Err(Failure::Other(reason))) => {
            state.stage = Stage::Ended(reason);
            Task::none()
        }
        Message::ReconnectPressed => state.begin(context),
        Message::Frame(picture) => {
            if let Stage::Connected(live) = &mut state.stage {
                live.size = (picture.width, picture.height);
                live.picture = picture;
            }
            Task::none()
        }
        Message::Stats(stats) => {
            if let Stage::Connected(live) = &mut state.stage {
                live.meter.sample(Totals {
                    received: stats.received,
                    sent: stats.sent,
                    frames: stats.frames,
                });
            }
            Task::none()
        }
        Message::Clipboard(text) => {
            let Stage::Connected(live) = &mut state.stage else {
                return Task::none();
            };
            // Remembered, so the next look at the local clipboard does not send it back.
            live.clipboard = Some(text.clone());
            iced::clipboard::write(text)
        }
        Message::Cursor(cursor) => {
            if let Stage::Connected(live) = &mut state.stage {
                live.cursor = match cursor {
                    Some(cursor) => Look::from_pixels(
                        cursor.width,
                        cursor.height,
                        (cursor.hotspot_x, cursor.hotspot_y),
                        cursor.rgba,
                    ),
                    None => Look::Hidden,
                };
            }
            Task::none()
        }
        Message::Closed(reason) => {
            state.stage = Stage::Ended(reason.unwrap_or_else(|| fl!("session-closed")));
            Task::none()
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
        Message::PointerOver(over) => {
            if let Stage::Connected(live) = &mut state.stage {
                live.over = over;
            }
            Task::none()
        }
        Message::DetailsToggled => {
            if let Stage::Connected(live) = &mut state.stage {
                live.details_open = !live.details_open;
            }
            Task::none()
        }
    }
}

impl State {
    /// The pointer position on the remote desktop, for presses that carry none.
    fn pointer(&self) -> (u16, u16) {
        match &self.stage {
            Stage::Connected(live) => live.pointer,
            _ => (0, 0),
        }
    }
}

#[cfg(test)]
#[path = "vnc_tests.rs"]
mod tests;
