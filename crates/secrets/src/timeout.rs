//! Keeps a store that never answers from freezing its caller.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use crate::{Error, Result, Secret, SecretStore};

/// Runs `operation` on its own thread and gives up waiting after `timeout`.
///
/// A timed-out operation is abandoned, not cancelled: its thread lives on
/// until the store answers, and the answer is dropped.
pub fn run_with_timeout<T, F>(timeout: Duration, operation: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        // The receiver is gone if we already gave up; nothing to do then.
        let _ = sender.send(operation());
    });

    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(_) => Err(Error::TimedOut(timeout.as_secs())),
    }
}

/// Wraps a store so that every call gives up after a fixed time.
pub struct TimeoutStore {
    inner: Arc<dyn SecretStore>,
    timeout: Duration,
}

impl TimeoutStore {
    /// Wraps `inner`; each call waits at most `timeout`.
    pub fn new(inner: Arc<dyn SecretStore>, timeout: Duration) -> Self {
        Self { inner, timeout }
    }
}

impl SecretStore for TimeoutStore {
    fn get(&self, credential_id: &str) -> Result<Option<Secret>> {
        let inner = Arc::clone(&self.inner);
        let id = credential_id.to_string();
        run_with_timeout(self.timeout, move || inner.get(&id))
    }

    fn set(&self, credential_id: &str, secret: &Secret) -> Result<()> {
        let inner = Arc::clone(&self.inner);
        let id = credential_id.to_string();
        let secret = secret.clone();
        run_with_timeout(self.timeout, move || inner.set(&id, &secret))
    }

    fn delete(&self, credential_id: &str) -> Result<()> {
        let inner = Arc::clone(&self.inner);
        let id = credential_id.to_string();
        run_with_timeout(self.timeout, move || inner.delete(&id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryStore;

    /// A store whose every call takes a fixed time, like a keyring waiting for a prompt.
    struct SlowStore {
        delay: Duration,
        inner: MemoryStore,
    }

    impl SecretStore for SlowStore {
        fn get(&self, credential_id: &str) -> Result<Option<Secret>> {
            std::thread::sleep(self.delay);
            self.inner.get(credential_id)
        }

        fn set(&self, credential_id: &str, secret: &Secret) -> Result<()> {
            std::thread::sleep(self.delay);
            self.inner.set(credential_id, secret)
        }

        fn delete(&self, credential_id: &str) -> Result<()> {
            std::thread::sleep(self.delay);
            self.inner.delete(credential_id)
        }
    }

    fn slow(delay_ms: u64, timeout_ms: u64) -> TimeoutStore {
        let inner = SlowStore {
            delay: Duration::from_millis(delay_ms),
            inner: MemoryStore::new(),
        };
        TimeoutStore::new(Arc::new(inner), Duration::from_millis(timeout_ms))
    }

    #[test]
    fn a_store_that_answers_in_time_works_as_usual() {
        let store = slow(1, 2000);

        store
            .set("cred-ops", &Secret::new("hunter2".to_string()))
            .unwrap();
        let read = store.get("cred-ops").unwrap().unwrap();
        store.delete("cred-ops").unwrap();

        assert_eq!(read.expose(), "hunter2");
        assert!(store.get("cred-ops").unwrap().is_none());
    }

    #[test]
    fn a_store_that_is_too_slow_times_out() {
        let store = slow(2000, 20);

        assert!(matches!(store.get("cred-ops"), Err(Error::TimedOut(_))));
        let secret = Secret::new("hunter2".to_string());
        assert!(matches!(
            store.set("cred-ops", &secret),
            Err(Error::TimedOut(_))
        ));
        assert!(matches!(store.delete("cred-ops"), Err(Error::TimedOut(_))));
    }

    #[test]
    fn an_error_from_the_operation_is_passed_through() {
        let result: Result<()> = run_with_timeout(Duration::from_secs(2), || {
            Err(Error::Backend("refused".to_string()))
        });

        assert!(matches!(result, Err(Error::Backend(reason)) if reason == "refused"));
    }
}
