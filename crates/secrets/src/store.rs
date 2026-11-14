use crate::{Result, Secret};

/// A place that keeps one secret per credential id.
///
/// The calls block, so run them off the UI thread.
pub trait SecretStore: Send + Sync {
    /// The secret for this credential, or `None` when nothing is stored.
    fn get(&self, credential_id: &str) -> Result<Option<Secret>>;

    /// Stores the secret, replacing any earlier one.
    fn set(&self, credential_id: &str, secret: &Secret) -> Result<()>;

    /// Removes the secret. Removing one that is not there is not an error.
    fn delete(&self, credential_id: &str) -> Result<()>;
}
