//! Why a VNC connection could not be made.

/// What can go wrong when connecting.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The server could not be reached.
    #[error("{host}:{port} could not be reached: {reason}")]
    Unreachable {
        /// The host that was tried.
        host: String,
        /// The port that was tried.
        port: u16,
        /// What the system said.
        reason: String,
    },
    /// The server asks for a password and none was given.
    #[error("the server asks for a password")]
    PasswordNeeded,
    /// The server refused the password.
    #[error("the server refused the password")]
    WrongPassword,
    /// The server spoke something unexpected or ended the connection.
    #[error("{0}")]
    Protocol(String),
}

/// A result with this module's error.
pub type Result<T> = std::result::Result<T, Error>;

impl From<vnc::VncError> for Error {
    fn from(error: vnc::VncError) -> Self {
        match error {
            vnc::VncError::NoPassword => Error::PasswordNeeded,
            vnc::VncError::WrongPassword => Error::WrongPassword,
            // TigerVNC and others refuse a bad password with this text, which vnc-rs passes on as is.
            vnc::VncError::General(reason)
                if reason.to_lowercase().contains("authentication failed") =>
            {
                Error::WrongPassword
            }
            other => Error::Protocol(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_password_is_told_apart_from_a_wrong_one() {
        assert!(matches!(
            Error::from(vnc::VncError::NoPassword),
            Error::PasswordNeeded
        ));
        assert!(matches!(
            Error::from(vnc::VncError::WrongPassword),
            Error::WrongPassword
        ));
    }

    #[test]
    fn a_refusal_in_words_is_a_wrong_password() {
        let error = Error::from(vnc::VncError::General("Authentication failed".to_string()));

        assert!(matches!(error, Error::WrongPassword));
    }

    #[test]
    fn anything_else_keeps_its_message() {
        let error = Error::from(vnc::VncError::General("boom".to_string()));

        assert!(matches!(&error, Error::Protocol(text) if text.contains("boom")));
    }
}
