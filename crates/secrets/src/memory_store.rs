use std::collections::HashMap;
use std::sync::Mutex;

use crate::{Result, Secret, SecretStore};

/// A store that forgets everything when the process ends.
///
/// Used in tests, and by the app for "remember until I quit".
#[derive(Default)]
pub struct MemoryStore {
    secrets: Mutex<HashMap<String, Secret>>,
}

impl MemoryStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    fn secrets(&self) -> std::sync::MutexGuard<'_, HashMap<String, Secret>> {
        // A panic while holding the lock cannot leave the map half-updated.
        self.secrets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, credential_id: &str) -> Result<Option<Secret>> {
        Ok(self.secrets().get(credential_id).cloned())
    }

    fn set(&self, credential_id: &str, secret: &Secret) -> Result<()> {
        self.secrets()
            .insert(credential_id.to_string(), secret.clone());
        Ok(())
    }

    fn delete(&self, credential_id: &str) -> Result<()> {
        self.secrets().remove(credential_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(secret: Option<Secret>) -> Option<String> {
        secret.map(|secret| secret.expose().to_string())
    }

    #[test]
    fn nothing_stored_reads_as_none() {
        let store = MemoryStore::new();

        assert_eq!(text(store.get("cred-ops").unwrap()), None);
    }

    #[test]
    fn a_stored_secret_can_be_read_back() {
        let store = MemoryStore::new();

        store
            .set("cred-ops", &Secret::new("hunter2".to_string()))
            .unwrap();

        assert_eq!(
            text(store.get("cred-ops").unwrap()).as_deref(),
            Some("hunter2")
        );
    }

    #[test]
    fn setting_again_replaces_the_secret() {
        let store = MemoryStore::new();

        store
            .set("cred-ops", &Secret::new("old".to_string()))
            .unwrap();
        store
            .set("cred-ops", &Secret::new("new".to_string()))
            .unwrap();

        assert_eq!(text(store.get("cred-ops").unwrap()).as_deref(), Some("new"));
    }

    #[test]
    fn secrets_are_kept_per_credential() {
        let store = MemoryStore::new();

        store
            .set("cred-ops", &Secret::new("one".to_string()))
            .unwrap();

        assert_eq!(text(store.get("cred-dev").unwrap()), None);
    }

    #[test]
    fn a_deleted_secret_is_gone() {
        let store = MemoryStore::new();
        store
            .set("cred-ops", &Secret::new("hunter2".to_string()))
            .unwrap();

        store.delete("cred-ops").unwrap();

        assert_eq!(text(store.get("cred-ops").unwrap()), None);
    }

    #[test]
    fn deleting_a_missing_secret_is_fine() {
        let store = MemoryStore::new();

        store.delete("cred-ops").unwrap();
    }
}
