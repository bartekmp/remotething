//! SSH: an interactive shell on a remote host.
//!
//! [`open_shell`] connects, checks the host key against the accepted ones,
//! signs in and starts a shell. The [`Shell`] it returns is two channels:
//! events from the remote side, and input to it. Dropping the input end
//! closes the connection.

mod connect;
mod host_key;
mod shell;

use std::path::PathBuf;

use remotething_secrets::Secret;
use tokio::sync::mpsc;

pub use connect::open_shell;
pub use shell::{Connection, SftpStream};

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

/// The public key a server identified itself with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKey {
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
