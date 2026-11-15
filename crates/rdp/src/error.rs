//! Why a session could not be opened.

/// A server certificate as shown to the user and pinned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    /// The host that was connected to.
    pub host: String,
    /// The port that was connected to.
    pub port: u16,
    /// What is stored when the user accepts it: the SHA-256 of the certificate.
    pub key: String,
    /// The same hash as upper-case hex pairs, for the user to compare.
    pub fingerprint: String,
}

/// Why a session could not be opened.
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

    /// The server's certificate has not been accepted yet. No credential was sent.
    #[error("the certificate of this host is not known yet ({})", .0.fingerprint)]
    UnknownCertificate(Certificate),

    /// The server's certificate differs from the accepted one. No credential was sent.
    #[error("the certificate of this host has changed ({})", .0.fingerprint)]
    CertificateChanged(Certificate),

    /// The server refused the user name or password.
    #[error("the server refused the user name or password")]
    AuthRefused,

    /// Anything else that went wrong while connecting or during the session.
    #[error("RDP failed: {0}")]
    Protocol(String),
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;
