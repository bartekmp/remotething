/// Everything that can go wrong when storing or fetching a secret.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// There is no usable keyring, for example no Secret Service on a headless session.
    #[error("no secret store is available: {0}")]
    Unavailable(String),

    /// The store did not answer in time, for example an unlock prompt nobody answered.
    #[error("the secret store did not answer within {0} seconds")]
    TimedOut(u64),

    /// The master password is wrong, or the encrypted file was altered.
    #[error("wrong master password, or the secrets file was altered")]
    WrongPassword,

    /// The store is there but refused or failed the operation.
    #[error("secret store failed: {0}")]
    Backend(String),

    /// The stored bytes are not valid text.
    #[error("the stored secret for \"{0}\" is not valid UTF-8")]
    NotText(String),
}
