//! An FTP or FTPS tab: signing in, the certificate question for FTPS, then the
//! files filling the tab. Drawing is in `ftp_view.rs`.

use std::sync::Arc;

use iced::Task;
use remotething_core::{CredentialKind, Protocol, ResolvedConnection};
use remotething_files::{Certificate, Error, FtpFs, FtpSettings, RemoteFs};
use remotething_secrets::Secret;

use crate::credentials::Hidden;
use crate::files_pane;
use crate::i18n::fl;
use crate::session::{Context, Sealed};

pub struct State {
    pub(crate) connection: ResolvedConnection,
    /// The user typed in a connection string, for a connection without a credential.
    typed_username: Option<String>,
    pub(crate) username: String,
    password: Option<Secret>,
    pub(crate) stage: Stage,
}

pub(crate) enum Stage {
    /// Reading the stored password.
    LoadingSecret,
    /// Waiting for the user to type what is missing.
    Asking(Form),
    Connecting,
    /// The FTPS server's certificate is new or has changed; the user decides.
    Certificate {
        certificate: Certificate,
        changed: bool,
    },
    /// Signed in: the files fill the tab.
    Browsing(Box<files_pane::State>),
    /// Closed or failed, with the reason shown.
    Ended(String),
}

pub(crate) struct Form {
    pub(crate) ask_username: bool,
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) note: Option<String>,
}

/// A signed-in file system on its way to the window.
#[derive(Clone)]
pub struct Opened(Arc<dyn RemoteFs>);

impl std::fmt::Debug for Opened {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Opened")
    }
}

/// Why opening did not work, in the shape the window acts on.
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
            Error::LoginRefused => Failure::Refused,
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
    TrustPressed,
    ReconnectPressed,
    Files(files_pane::Message),
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
            password: None,
            stage: Stage::Connecting,
        };
        let task = state.begin(context);
        (state, task)
    }

    /// Whether the files are showing, so closing the tab would cut the connection.
    pub(crate) fn is_connected(&self) -> bool {
        matches!(self.stage, Stage::Browsing(_))
    }

    /// Whether the connection protects the password and the files with TLS.
    pub(crate) fn is_encrypted(&self) -> bool {
        self.connection.protocol == Protocol::Ftps
    }

    /// Puts the tab into the browsing stage without a server.
    #[cfg(test)]
    pub(crate) fn browse_for_test(&mut self) {
        let mut files = files_pane::State::empty();
        files.closable = false;
        self.stage = Stage::Browsing(Box::new(files));
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
                self.stage = Stage::Ended(fl!("ftp-credential-kind"));
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
        let secure = self.is_encrypted().then(|| context.known_hosts.clone());
        let settings = FtpSettings {
            host: self.connection.host.clone(),
            port: self.connection.port,
            username: self.username.clone(),
            password,
            secure,
        };
        self.stage = Stage::Connecting;
        Task::perform(open(settings), Message::Opened)
    }

    fn go_live(&mut self, opened: Opened) -> Task<Message> {
        let (mut pane, task) = files_pane::State::browse(opened.0);
        // The tab is the browser: there is nothing behind the pane to go back to.
        pane.closable = false;
        self.stage = Stage::Browsing(Box::new(pane));
        task.map(Message::Files)
    }
}

async fn open(settings: FtpSettings) -> Result<Opened, Failure> {
    let files = FtpFs::connect(settings).await.map_err(Failure::from)?;
    let files: Arc<dyn RemoteFs> = Arc::new(files);
    Ok(Opened(files))
}

pub fn update(state: &mut State, message: Message, context: &mut Context<'_>) -> Task<Message> {
    match message {
        Message::SecretLoaded(Ok(Sealed(Some(secret)))) => {
            state.password = Some(secret);
            state.connect(context)
        }
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
            state.password = Some(Secret::new(std::mem::take(&mut form.password)));
            state.username = username;
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
        Message::TrustPressed => {
            let Stage::Certificate { certificate, .. } = &state.stage else {
                return Task::none();
            };
            let trusted =
                context
                    .known_hosts
                    .trust(&certificate.host, certificate.port, &certificate.key);
            match trusted {
                Ok(()) => state.connect(context),
                Err(error) => {
                    let reason = format!("{} {error}", fl!("session-trust-failed"));
                    state.stage = Stage::Ended(reason);
                    Task::none()
                }
            }
        }
        Message::ReconnectPressed => state.begin(context),
        Message::Files(message) => match &mut state.stage {
            Stage::Browsing(pane) => files_pane::update(pane, message).0.map(Message::Files),
            _ => Task::none(),
        },
    }
}

#[cfg(test)]
#[path = "ftp_tab_tests.rs"]
mod tests;
