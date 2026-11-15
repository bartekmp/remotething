//! A Windows Remote Desktop tab: signing in, the certificate question and the
//! live picture with the user's mouse and keyboard going back.

use std::sync::Arc;

use iced::futures::stream;
use iced::{Size, Task};
use remotething_core::{CredentialKind, ResolvedConnection};
use remotething_rdp::{Button, Certificate, Event, Info, Input, Session, Stats, Target};
use remotething_secrets::Secret;
use tokio::sync::mpsc;

use crate::credentials::Hidden;
use crate::desktop_open::{Failure, Opened};
use crate::desktop_size;
use crate::display;
use crate::i18n::fl;
use crate::meter::{Meter, Totals};
use crate::picture::Picture;
use crate::quality_choice;
use crate::scancodes::Scancode;
use crate::session::{Context, Sealed};

/// The size of the remote desktop until resizing is supported.
const WIDTH: u16 = 1280;
const HEIGHT: u16 = 800;
/// How long Ctrl+Alt+Del is held; Windows ignores a shorter press.
const CTRL_ALT_DEL_HOLD: std::time::Duration = std::time::Duration::from_millis(300);
/// How long the room for the picture must stay the same before the server is asked to match it.
const RESIZE_SETTLE: std::time::Duration = std::time::Duration::from_millis(400);

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
    pub(crate) picture: Picture,
    /// The size of the remote desktop in pixels.
    pub(crate) size: (u32, u32),
    input: mpsc::Sender<Input>,
    pub(crate) info: Info,
    pub(crate) meter: Meter,
    pub(crate) details_open: bool,
    /// The latest room for the picture.
    area: Size,
    /// The clipboard text last seen locally or received from the server.
    clipboard: Option<String>,
    /// Where the pointer is on the remote desktop, for button presses.
    pointer: (u16, u16),
}

#[derive(Debug, Clone)]
pub enum Message {
    SecretLoaded(Result<Sealed, String>),
    UsernameChanged(String),
    PasswordChanged(Hidden),
    Submitted,
    Opened(Result<Opened, Failure>),
    /// The desktop changed: a new picture and its size.
    Frame(Picture),
    /// Running totals of the traffic.
    Stats(Stats),
    /// Text was copied on the server.
    Clipboard(String),
    /// The (i) button was pressed.
    DetailsToggled,
    /// The Ctrl+Alt+Del button was pressed.
    CtrlAltDelPressed,
    /// The keys of Ctrl+Alt+Del were held long enough.
    CtrlAltDelReleased,
    /// The room for the picture changed.
    AreaChanged(Size),
    /// The room stayed the same for a moment, so it is worth asking the server.
    AreaSettled(Size),
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
            picture: Picture::black(),
            size: (1280, 800),
            input,
            info: Info {
                peer: "127.0.0.1:3389".parse().expect("a valid address"),
                codec: "test",
                security: "test",
            },
            meter: Meter::new(),
            details_open: false,
            area: Size::ZERO,
            clipboard: None,
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
            quality: quality_choice::for_session(self.connection.rdp_quality),
            scale: display::scale_percent(),
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
                    Ok(Ok(session)) => Ok(Opened::new(session)),
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
        let Session {
            info,
            events,
            input,
        } = session;
        self.stage = Stage::Connected(Box::new(Live {
            // Black until the first picture arrives.
            picture: Picture::black(),
            size: (u32::from(WIDTH), u32::from(HEIGHT)),
            input,
            info,
            meter: Meter::new(),
            details_open: false,
            area: Size::ZERO,
            clipboard: None,
            pointer: (0, 0),
        }));

        // The stream ends when the session thread stops, for example once this tab is closed.
        let messages = stream::unfold(events, |mut events| async move {
            let message = match events.recv().await? {
                Event::Frame(frame) => {
                    Message::Frame(Picture::new(frame.width, frame.height, frame.rgba))
                }
                Event::Stats(stats) => Message::Stats(stats),
                Event::Clipboard(text) => Message::Clipboard(text),
                Event::Closed(reason) => Message::Closed(reason),
            };
            Some((message, events))
        });
        Task::run(messages, |message| message)
    }

    /// The screen scaling changed: ask for a desktop of the new real size.
    pub(crate) fn rescaled(&mut self) {
        let Stage::Connected(live) = &self.stage else {
            return;
        };
        if let Some((width, height)) = desktop_size::wanted(live.area, display::scale(), live.size)
        {
            self.send(Input::Resize {
                width,
                height,
                scale: display::scale_percent(),
            });
        }
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
                    sent: 0,
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
        Message::DetailsToggled => {
            if let Stage::Connected(live) = &mut state.stage {
                live.details_open = !live.details_open;
            }
            Task::none()
        }
        Message::CtrlAltDelPressed => {
            for key in remotething_rdp::ctrl_alt_delete(true) {
                state.send(key);
            }
            Task::perform(
                async { tokio::time::sleep(CTRL_ALT_DEL_HOLD).await },
                |()| Message::CtrlAltDelReleased,
            )
        }
        Message::CtrlAltDelReleased => {
            for key in remotething_rdp::ctrl_alt_delete(false) {
                state.send(key);
            }
            Task::none()
        }
        Message::AreaChanged(area) => {
            let Stage::Connected(live) = &mut state.stage else {
                return Task::none();
            };
            live.area = area;
            // A window being dragged reports many sizes; only the last one counts.
            Task::perform(
                async move {
                    tokio::time::sleep(RESIZE_SETTLE).await;
                    area
                },
                Message::AreaSettled,
            )
        }
        Message::AreaSettled(area) => {
            if let Stage::Connected(live) = &state.stage
                && live.area == area
                && let Some((width, height)) =
                    desktop_size::wanted(area, display::scale(), live.size)
            {
                state.send(Input::Resize {
                    width,
                    height,
                    scale: display::scale_percent(),
                });
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
