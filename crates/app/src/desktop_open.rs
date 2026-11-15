//! How opening a desktop session ends, in the shape the window acts on.

use std::sync::{Arc, Mutex};

use remotething_rdp::{Certificate, Error, Session};

/// A freshly opened session on its way to the window. It is taken out once.
#[derive(Clone)]
pub struct Opened(pub(super) Arc<Mutex<Option<Session>>>);

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

impl Opened {
    /// Wraps a session for the trip through a message.
    pub(super) fn new(session: Session) -> Self {
        Self(Arc::new(Mutex::new(Some(session))))
    }
}
