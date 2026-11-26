//! An FTP, FTPS, SMB or NFS tab: signing in, the certificate question for FTPS, then the
//! files filling the tab. Drawing is in `ftp_view.rs`.

use std::sync::Arc;

use iced::Task;
use iced::futures::FutureExt;
use remotething_core::{CredentialKind, Protocol, ResolvedConnection};
use remotething_files::{
    Certificate, Error, FtpFs, FtpSettings, NOBODY, NfsFs, NfsSettings, RemoteFs, SmbFs,
    SmbSettings,
};
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
    /// The domain of the account, for SMB.
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
            domain: None,
            password: None,
            stage: Stage::Connecting,
        };
        let task = state.begin(context);
        (state, task)
    }

    /// Whether the files are showing, so closing the tab would cut the connection.
    /// Whether a sign-in form is showing, where Tab moves between the fields.
    pub(crate) fn is_asking(&self) -> bool {
        matches!(self.stage, Stage::Asking(_))
    }

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

    /// A key in the files, when they are showing.
    pub(crate) fn key(
        &mut self,
        key: &iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    ) -> Task<Message> {
        match &mut self.stage {
            Stage::Browsing(pane) => {
                crate::files_keys::on_key(pane, key, modifiers).map(Message::Files)
            }
            _ => Task::none(),
        }
    }

    fn begin(&mut self, context: &Context<'_>) -> Task<Message> {
        if self.connection.protocol == Protocol::Nfs {
            return self.connect_nfs(context);
        }
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
                self.stage = Stage::Ended(fl!("ftp-credential-kind"));
                Task::none()
            }
        }
    }

    /// NFS has no password: the server trusts the user and group number. They are written as
    /// the user name of the credential (`1000` or `1000:100`), or typed in a connection string.
    fn connect_nfs(&mut self, context: &Context<'_>) -> Task<Message> {
        let named = self
            .connection
            .credential
            .as_ref()
            .and_then(|id| context.store.credentials.iter().find(|c| &c.id == id))
            .and_then(|credential| credential.username.clone())
            .or_else(|| self.typed_username.clone())
            .unwrap_or_default();
        let (uid, gid) = nfs_identity(&named);
        self.username = format!("{uid}:{gid}");
        let settings = NfsSettings {
            host: self.connection.host.clone(),
            port: self.connection.port,
            uid,
            gid,
        };
        self.stage = Stage::Connecting;
        Task::perform(open_nfs(settings), Message::Opened)
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
        let host = self.connection.host.clone();
        let port = self.connection.port;
        let username = self.username.clone();
        let opening = if self.connection.protocol == Protocol::Smb {
            let settings = SmbSettings {
                host,
                port,
                username,
                password,
                domain: self.domain.clone(),
            };
            open_smb(settings).boxed()
        } else {
            let secure = self.is_encrypted().then(|| context.known_hosts.clone());
            let settings = FtpSettings {
                host,
                port,
                username,
                password,
                secure,
            };
            open(settings).boxed()
        };
        self.stage = Stage::Connecting;
        Task::perform(opening, Message::Opened)
    }

    fn go_live(&mut self, opened: Opened) -> Task<Message> {
        let (mut pane, task) = files_pane::State::browse(opened.0);
        // The tab is the browser: there is nothing behind the pane to go back to.
        pane.closable = false;
        self.stage = Stage::Browsing(Box::new(pane));
        task.map(Message::Files)
    }
}

/// The user and group number in `1000` or `1000:100` (the group then is the user's number
/// again); anything else is the unprivileged "nobody".
fn nfs_identity(text: &str) -> (u32, u32) {
    let text = text.trim();
    let (user, group) = text.split_once(':').unwrap_or((text, text));
    match (user.parse(), group.parse()) {
        (Ok(user), Ok(group)) => (user, group),
        _ => (NOBODY, NOBODY),
    }
}

async fn open_nfs(settings: NfsSettings) -> Result<Opened, Failure> {
    let files = NfsFs::connect(settings).await.map_err(Failure::from)?;
    let files: Arc<dyn RemoteFs> = Arc::new(files);
    Ok(Opened(files))
}

async fn open_smb(settings: SmbSettings) -> Result<Opened, Failure> {
    let files = SmbFs::connect(settings).await.map_err(Failure::from)?;
    let files: Arc<dyn RemoteFs> = Arc::new(files);
    Ok(Opened(files))
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
        Message::SecretLoaded(Ok(Sealed(None))) => state.ask(Some(fl!("session-secret-missing"))),
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
