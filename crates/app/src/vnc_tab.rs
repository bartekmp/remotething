//! A VNC tab: asking for the password, then the live picture with the user's
//! mouse and keyboard going back.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use iced::Task;
use iced::futures::stream;
use iced::keyboard::Key;
use iced::keyboard::key::Physical;
use remotething_core::{CredentialKind, ResolvedConnection};
use remotething_remote::ssh::{self, Endpoint, HostKey};
use remotething_secrets::Secret;
use remotething_vnc::{Button, Error, Event, Info, Input, Session, Stats, Target};
use tokio::sync::mpsc;

use crate::credentials::Hidden;
use crate::desktop_cursor::Look;
use crate::features;
use crate::i18n::fl;
use crate::jump;
use crate::keysym;
use crate::meter::{Meter, Totals};
use crate::note::Note;
use crate::picture::Picture;
use crate::quality_choice;
use crate::screenshot;
use crate::session::{Context, Sealed, SealedJumps};
use crate::video_recording::Video;

pub struct State {
    pub(crate) connection: ResolvedConnection,
    password: Option<Secret>,
    /// The SSH servers the connection is tunnelled through, in the order they are connected to.
    jumps: Vec<jump::Ready>,
    pub(crate) stage: Stage,
}

pub(crate) enum Stage {
    /// Reading the stored password.
    LoadingSecret,
    /// The server wants a password and none is known (or it was refused).
    Asking(Form),
    Connecting,
    /// A server on the way has a key that is new or has changed; the user decides.
    HostKey {
        key: HostKey,
        changed: bool,
    },
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
    /// How the last screenshot went, for a few seconds.
    pub(crate) note: Option<Note>,
    /// The recording of this desktop, if there is one.
    pub(crate) video: Video,
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
    HostKey { key: HostKey, changed: bool },
    PasswordNeeded,
    WrongPassword,
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
            other => Failure::Other(other.to_string()),
        }
    }
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
    /// The stored secrets of the SSH servers on the way were read, or one is missing.
    JumpsLoaded(Result<SealedJumps, String>),
    SecretLoaded(Result<Sealed, String>),
    TrustPressed,
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
    RecordPressed,
    ScreenshotPressed,
    ScreenshotSaved(Result<String, String>),
}

impl State {
    /// Works out how to sign in and starts as far as it can without the user.
    pub fn start(connection: ResolvedConnection, context: &Context<'_>) -> (Self, Task<Message>) {
        let mut state = State {
            connection,
            password: None,
            jumps: Vec::new(),
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

    /// The SSH servers the connection goes through.
    pub(crate) fn jumps(&self) -> impl Iterator<Item = &jump::Ready> {
        self.jumps.iter()
    }

    /// Takes the next slot of the recording, if one is going.
    pub(crate) fn tick_recording(&mut self) {
        if let Stage::Connected(live) = &mut self.stage {
            live.video.tick(&live.picture);
        }
    }

    /// Whether a recording is going.
    pub(crate) fn is_recording(&self) -> bool {
        matches!(&self.stage, Stage::Connected(live) if live.video.is_active())
    }

    /// Whether a desktop is showing, so closing the tab would cut it.
    /// Whether a sign-in form is showing, where Tab moves between the fields.
    pub(crate) fn is_asking(&self) -> bool {
        matches!(self.stage, Stage::Asking(_))
    }

    pub(crate) fn is_connected(&self) -> bool {
        matches!(self.stage, Stage::Connected(_))
    }

    /// Starts over: works out the SSH servers to tunnel through first, then how to sign in.
    fn begin(&mut self, context: &Context<'_>) -> Task<Message> {
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

    /// Works out how to sign in to the VNC server and starts as far as it can without the user.
    fn begin_target(&mut self, context: &Context<'_>) -> Task<Message> {
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
            return self.connect(context);
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

    /// Connects with what is known, directly or through the SSH servers on the way.
    fn connect(&mut self, context: &Context<'_>) -> Task<Message> {
        let target = Target {
            host: self.connection.host.clone(),
            port: self.connection.port,
            password: self.password.clone(),
            quality: quality_choice::for_vnc(self.connection.desktop_quality),
        };
        let hops: Vec<ssh::Hop> = self.jumps.iter().filter_map(jump::Ready::hop).collect();
        let known = context.known_hosts.clone();
        self.stage = Stage::Connecting;
        Task::perform(
            async move {
                let opened = if hops.is_empty() {
                    remotething_vnc::connect(target).await
                } else {
                    let endpoint = Endpoint {
                        host: target.host,
                        port: target.port,
                    };
                    let tunnel = ssh::open_tunnel(&hops, &endpoint, known)
                        .await
                        .map_err(Failure::from)?;
                    remotething_vnc::connect_over(tunnel, target.password, target.quality, None)
                        .await
                };
                opened
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
            note: None,
            video: Video::default(),
            held: HashMap::new(),
        }
    }
}

pub fn update(state: &mut State, message: Message, context: &mut Context<'_>) -> Task<Message> {
    match message {
        Message::JumpsLoaded(Ok(SealedJumps(jumps))) => {
            // Typing a jump host's password is only built for SSH tabs; never skip the jump.
            if let Some(asked) = jumps.iter().find(|jump| jump.needs_typing()) {
                state.stage = Stage::Ended(fl!("jump-ask-unsupported", name = asked.name.as_str()));
                return Task::none();
            }
            state.jumps = jumps;
            state.begin_target(context)
        }
        Message::JumpsLoaded(Err(reason)) => {
            state.stage = Stage::Ended(reason);
            Task::none()
        }
        Message::SecretLoaded(Ok(Sealed(Some(secret)))) => {
            state.password = Some(secret);
            state.connect(context)
        }
        // A password credential with nothing stored: it has to be typed.
        Message::SecretLoaded(Ok(Sealed(None))) => state.ask(Some(fl!("session-secret-missing"))),
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
            let Stage::Asking(form) = &mut state.stage else {
                return Task::none();
            };
            // Moved, not copied: no second copy of the typed password is left in the form.
            state.password = Some(Secret::new(std::mem::take(&mut form.password)));
            state.connect(context)
        }
        Message::Opened(Ok(opened)) => state.go_live(opened),
        Message::Opened(Err(Failure::HostKey { key, changed })) => {
            state.stage = Stage::HostKey { key, changed };
            Task::none()
        }
        Message::TrustPressed => {
            let Stage::HostKey { key, .. } = &state.stage else {
                return Task::none();
            };
            // The key belongs to an SSH server on the way, not to the VNC server.
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
                    // An empty shape means the server shows no pointer, which on a machine
                    // with no mouse is what Windows does, so the local arrow stays.
                    None => Look::Local,
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
        Message::RecordPressed => {
            if let Stage::Connected(live) = &mut state.stage {
                live.video.toggle(&state.connection.name, &live.picture);
            }
            Task::none()
        }
        Message::ScreenshotPressed => {
            let Stage::Connected(live) = &state.stage else {
                return Task::none();
            };
            Task::perform(
                screenshot::save_picture(
                    features::screenshots_folder(),
                    state.connection.name.clone(),
                    live.picture.width,
                    live.picture.height,
                    live.picture.pixels(),
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
