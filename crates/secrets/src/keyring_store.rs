use keyring::Entry;

use crate::{Error, Result, Secret, SecretStore};

/// All our entries share this service name; the credential id is the account.
const SERVICE: &str = "remotething";

/// The operating system's keyring: Secret Service on Linux, Credential Manager
/// on Windows, Keychain on macOS.
///
/// A locked keyring makes the desktop show an unlock prompt and the call waits
/// for it. On a session with no desktop that wait never ends, so callers must
/// run these calls off the UI thread and give up after a timeout.
pub struct KeyringStore;

impl KeyringStore {
    /// Connects to the platform keyring, or explains why there is none.
    pub fn open() -> Result<Self> {
        match Entry::store_status() {
            Ok(()) => Ok(Self),
            Err(error) => Err(Error::Unavailable(error.to_string())),
        }
    }

    fn entry(credential_id: &str) -> Result<Entry> {
        Entry::new(SERVICE, credential_id).map_err(convert)
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, credential_id: &str) -> Result<Option<Secret>> {
        match Self::entry(credential_id)?.get_password() {
            Ok(text) => Ok(Some(Secret::new(text))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(keyring::Error::BadEncoding(_)) => Err(Error::NotText(credential_id.to_string())),
            Err(error) => Err(convert(error)),
        }
    }

    fn set(&self, credential_id: &str, secret: &Secret) -> Result<()> {
        Self::entry(credential_id)?
            .set_password(secret.expose())
            .map_err(convert)
    }

    fn delete(&self, credential_id: &str) -> Result<()> {
        match Self::entry(credential_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(convert(error)),
        }
    }
}

/// Keyring errors describe the entry and the platform failure, never the secret.
fn convert(error: keyring::Error) -> Error {
    match error {
        keyring::Error::NoDefaultStore | keyring::Error::NoStorageAccess(_) => {
            Error::Unavailable(error.to_string())
        }
        other => Error::Backend(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a real, unlocked keyring, so it does not run in CI.
    /// Run with: cargo nextest run -p remotething-secrets --run-ignored all
    #[test]
    #[ignore = "needs an unlocked desktop keyring"]
    fn a_secret_survives_a_round_trip_through_the_real_keyring() {
        let store = KeyringStore::open().unwrap();
        let id = "remotething-test-round-trip";

        store.set(id, &Secret::new("hunter2".to_string())).unwrap();
        let read = store
            .get(id)
            .unwrap()
            .map(|secret| secret.expose().to_string());
        store.delete(id).unwrap();

        assert_eq!(read.as_deref(), Some("hunter2"));
        assert!(store.get(id).unwrap().is_none());
    }
}
