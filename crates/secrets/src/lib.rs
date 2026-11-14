//! Secrets for Remotething: passwords and key passphrases.
//!
//! A [`Secret`] cannot be printed or logged and is wiped from memory when
//! dropped. A [`SecretStore`] keeps secrets under the id of the credential
//! they belong to; the connections file only ever holds that id.

mod error;
mod file_store;
mod keyring_store;
mod memory_store;
mod secret;
mod store;
mod timeout;

pub use error::Error;
pub use file_store::FileStore;
pub use keyring_store::KeyringStore;
pub use memory_store::MemoryStore;
pub use secret::Secret;
pub use store::SecretStore;
pub use timeout::{TimeoutStore, run_with_timeout};

/// The result type used across this crate.
pub type Result<T> = std::result::Result<T, Error>;
