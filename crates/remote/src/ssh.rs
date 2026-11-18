//! SSH: an interactive shell on a remote host.
//!
//! [`open_shell`] connects (through jump hosts with [`open_shell_through`]),
//! checks every host key against the accepted ones,
//! signs in and starts a shell. The [`Shell`] it returns is two channels:
//! events from the remote side, and input to it. Dropping the input end
//! closes the connection.

mod connect;
mod forward;
mod host_key;
mod shell;
mod tunnel;

use std::path::PathBuf;

use remotething_secrets::Secret;
use tokio::sync::{mpsc, oneshot};

pub use connect::{open_connection_through, open_shell, open_shell_through};
pub use forward::Forwarding;
pub use shell::{Connection, Output, SftpStream};
pub use tunnel::{Tunnel, open_tunnel};

/// Where the SSH server listens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// A host name or an IP address.
    pub host: String,
    /// The TCP port, usually 22.
    pub port: u16,
}

/// How to sign in.
pub enum Auth {
    /// A user name and a password.
    Password {
        /// The account on the server.
        username: String,
        /// Its password.
        password: Secret,
    },
    /// Keys held by a running ssh-agent (on Windows the OpenSSH agent or Pageant).
    /// The agent signs; the private keys never reach this app.
    Agent {
        /// The account on the server.
        username: String,
        /// The agent's socket. `None` uses the one the environment names
        /// (`SSH_AUTH_SOCK`); on Windows `None` tries the OpenSSH pipe, then Pageant.
        socket: Option<PathBuf>,
    },
    /// Keyboard-interactive: the server asks questions (password, one-time
    /// code) and each is sent as a [`Challenge`] for the user to answer.
    Interactive {
        /// The account on the server.
        username: String,
        /// Where the questions go. The answer to each comes back on the challenge.
        challenges: mpsc::Sender<Challenge>,
    },
    /// A private key file in OpenSSH or PEM format.
    KeyFile {
        /// The account on the server.
        username: String,
        /// The private key file.
        path: PathBuf,
        /// The passphrase, if the key file is encrypted.
        passphrase: Option<Secret>,
    },
}

/// One question of a keyboard-interactive sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// The text the server shows, like `Verification code: `.
    pub text: String,
    /// Whether what is typed may be shown. False for passwords and codes.
    pub echo: bool,
}

/// What the server asks in one round of a keyboard-interactive sign-in.
pub struct Challenge {
    /// The server's title for the round; often empty.
    pub name: String,
    /// The server's instructions; often empty.
    pub instructions: String,
    /// The questions, in order.
    pub prompts: Vec<Prompt>,
    /// Send one answer per prompt here. Dropping it cancels the sign-in.
    pub reply: oneshot::Sender<Vec<Secret>>,
}

/// A server to connect through on the way to the target, and how to sign in to it.
pub struct Hop {
    /// Where the jump host listens.
    pub endpoint: Endpoint,
    /// How to sign in to it.
    pub auth: Auth,
}

/// What is known about an open connection, for showing to the user.
#[derive(Debug, Clone, Default)]
pub struct Details {
    /// The server's address. `None` when the connection runs through a jump host.
    pub peer: Option<std::net::SocketAddr>,
    /// How the server calls itself, for example `SSH-2.0-OpenSSH_9.6`.
    pub server: String,
    /// The algorithm of the server's host key.
    pub host_key_algorithm: String,
    /// The SHA-256 fingerprint of the server's host key.
    pub fingerprint: String,
    /// Remote forwards that are open: the port on the server and the host and port here
    /// that its clients are sent to. The handler reads it when the server opens a channel.
    pub(super) remote_targets: std::collections::HashMap<u32, (String, u16)>,
}

/// The public key a server identified itself with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKey {
    /// The server the key belongs to: the target, or a jump host on the way.
    pub endpoint: Endpoint,
    /// The key as one line of text, such as `ssh-ed25519 AAAA...`.
    pub key: String,
    /// The SHA-256 fingerprint to show to the user, such as `SHA256:...`.
    pub fingerprint: String,
}

/// The size of the terminal in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    /// Characters per line.
    pub columns: u16,
    /// Lines on screen.
    pub lines: u16,
}

/// What the local side sends to the shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellInput {
    /// Bytes typed or pasted.
    Data(Vec<u8>),
    /// The terminal view changed size.
    Resize(TerminalSize),
}

/// What the shell reports back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    /// Bytes to feed to the terminal, escape sequences included.
    Output(Vec<u8>),
    /// The shell ended, with its exit status if the server sent one.
    Closed(Option<u32>),
}

/// A running remote shell.
pub struct Shell {
    /// Output and the end of the session, in order.
    pub events: mpsc::Receiver<ShellEvent>,
    /// Typed bytes and resizes. Dropping it ends the shell.
    pub input: mpsc::Sender<ShellInput>,
    /// The connection the shell runs on, for opening more channels on it.
    pub connection: Connection,
}

/// Why a shell could not be opened.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No TCP connection could be made.
    #[error("cannot reach {host} on port {port}: {reason}")]
    Unreachable {
        /// The host that was tried.
        host: String,
        /// The port that was tried.
        port: u16,
        /// What the operating system said.
        reason: String,
    },

    /// The server did not finish connecting and signing in in time.
    #[error("the server did not answer within {0} seconds")]
    TimedOut(u64),

    /// The server's key has not been accepted yet. Nothing was sent to it.
    #[error("the key of this host is not known yet ({})", .0.fingerprint)]
    UnknownHostKey(HostKey),

    /// The server's key differs from the accepted one. Nothing was sent to it.
    #[error("the key of this host has changed ({})", .0.fingerprint)]
    HostKeyChanged(HostKey),

    /// The server refused the user name, password or key.
    #[error("the server refused the sign-in")]
    AuthRefused,

    /// No ssh-agent could be reached.
    #[error("no ssh-agent is running: {0}")]
    AgentUnavailable(String),

    /// The ssh-agent holds no keys.
    #[error("the ssh-agent holds no keys; add one with ssh-add")]
    AgentEmpty,

    /// The user dismissed the questions of a keyboard-interactive sign-in.
    #[error("the sign-in was cancelled")]
    SignInCancelled,

    /// The key file is encrypted and no passphrase was given.
    #[error("the key file {0} needs its passphrase")]
    KeyLocked(PathBuf),

    /// The key file could not be read, or the passphrase is wrong.
    #[error("cannot use the key file {path}: {reason}")]
    KeyFile {
        /// The private key file.
        path: PathBuf,
        /// Why it could not be used.
        reason: String,
    },

    /// The jump host is configured not to relay connections (`AllowTcpForwarding no`).
    #[error("{host} does not allow forwarding connections, so it cannot be used as a jump host")]
    ForwardingRefused {
        /// The jump host that refused.
        host: String,
    },

    /// A jump host on the way could not be used.
    #[error("jump host {host}: {reason}")]
    Jump {
        /// The jump host that failed.
        host: String,
        /// What went wrong there.
        reason: Box<Error>,
    },

    /// The server refused to listen on a port for a remote forward.
    #[error("the server would not listen on port {port}")]
    RemoteForwardRefused {
        /// The port on the server.
        port: u16,
    },

    /// The local port of a forward could not be listened on, usually because it is in use.
    #[error("cannot listen on port {port}: {reason}")]
    ListenFailed {
        /// The local port.
        port: u16,
        /// What the operating system said.
        reason: String,
    },

    /// Anything else the SSH layer reported.
    #[error("SSH failed: {0}")]
    Protocol(String),
}

impl From<russh::Error> for Error {
    fn from(error: russh::Error) -> Self {
        Error::Protocol(error.to_string())
    }
}

/// The result type of this module.
pub type Result<T> = std::result::Result<T, Error>;
