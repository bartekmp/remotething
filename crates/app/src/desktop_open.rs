//! How opening a desktop session ends, in the shape the window acts on.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use remotething_rdp::{Certificate, Error, Session, Target};
use remotething_secrets::KnownHosts;

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

/// How long to wait before the second try.
const RETRY_AFTER: Duration = Duration::from_secs(3);

/// Whether a failed connect is worth one more try. A server that has only just
/// let go of the last session sometimes drops the next connection half way
/// through the handshake; a refused password or an unknown certificate is not that.
fn worth_retrying(error: &Error) -> bool {
    matches!(error, Error::Protocol(_))
}

/// Connects, and tries once more after a moment when the server dropped the attempt midway.
pub(super) async fn open(target: Target, known: KnownHosts) -> Result<Opened, Failure> {
    let attempt = |target: Target, known: KnownHosts| async move {
        // Connecting blocks until the desktop is up.
        tokio::task::spawn_blocking(move || remotething_rdp::connect(target, &known)).await
    };
    let mut result = attempt(target.clone(), known.clone()).await;
    if matches!(&result, Ok(Err(error)) if worth_retrying(error)) {
        tokio::time::sleep(RETRY_AFTER).await;
        result = attempt(target, known).await;
    }
    match result {
        Ok(Ok(session)) => Ok(Opened::new(session)),
        Ok(Err(error)) => Err(Failure::from(error)),
        Err(error) => Err(Failure::Other(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_handshake_is_retried() {
        assert!(worth_retrying(&Error::Protocol("dropped".to_string())));
    }

    #[test]
    fn a_refused_password_is_not_retried() {
        assert!(!worth_retrying(&Error::AuthRefused));
    }

    #[test]
    fn an_unreachable_host_is_not_retried() {
        let error = Error::Unreachable {
            host: "x".to_string(),
            port: 3389,
            reason: "no route".to_string(),
        };

        assert!(!worth_retrying(&error));
    }
}
