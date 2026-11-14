//! Secrets in one encrypted file, for machines without a keyring.
//!
//! The key comes from a master password through Argon2id. The secrets are
//! sealed with XChaCha20-Poly1305, with a fresh random nonce on every write.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{Error, Result, Secret, SecretStore};

const FORMAT_VERSION: u32 = 1;
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 24;
const KEY_LENGTH: usize = 32;
/// Upper bounds for settings read from a file, so a damaged one cannot exhaust memory.
const MAX_MEMORY_KIB: u32 = 1024 * 1024;
const MAX_ITERATIONS: u32 = 64;
/// Bound into every seal, so a file of another kind or version cannot be decrypted as ours.
const ASSOCIATED_DATA: &[u8] = b"remotething-secrets-v1";

/// What is written to disk. Everything here is safe to read; only `ciphertext` is secret.
#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    /// Argon2id memory cost in KiB.
    memory_kib: u32,
    /// Argon2id passes.
    iterations: u32,
    /// Argon2id lanes.
    parallelism: u32,
    salt: String,
    nonce: String,
    ciphertext: String,
}

/// The key-derivation settings, stored in the file so they can be raised later.
#[derive(Clone, Copy)]
struct Cost {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl Cost {
    /// The `argon2` crate's defaults: 19 MiB, 2 passes, 1 lane (the OWASP minimum).
    const DEFAULT: Cost = Cost {
        memory_kib: Params::DEFAULT_M_COST,
        iterations: Params::DEFAULT_T_COST,
        parallelism: Params::DEFAULT_P_COST,
    };
}

type Key = Zeroizing<[u8; KEY_LENGTH]>;

/// An encrypted file holding every secret, unlocked by a master password.
pub struct FileStore {
    path: PathBuf,
    key: Key,
    salt: [u8; SALT_LENGTH],
    cost: Cost,
    secrets: Mutex<BTreeMap<String, Secret>>,
}

impl FileStore {
    /// Creates a new, empty store file. An existing file is refused, not overwritten.
    pub fn create(path: &Path, master_password: &Secret) -> Result<FileStore> {
        if path.exists() {
            return Err(Error::Backend(format!("{} already exists", path.display())));
        }
        let mut salt = [0u8; SALT_LENGTH];
        fill_random(&mut salt)?;
        let cost = Cost::DEFAULT;

        let store = FileStore {
            path: path.to_path_buf(),
            key: derive_key(master_password, &salt, cost)?,
            salt,
            cost,
            secrets: Mutex::new(BTreeMap::new()),
        };
        store.write(&BTreeMap::new())?;
        Ok(store)
    }

    /// Opens an existing store file with its master password.
    pub fn open(path: &Path, master_password: &Secret) -> Result<FileStore> {
        let text = fs::read_to_string(path).map_err(|error| io_error(path, error))?;
        let envelope: Envelope = toml::from_str(&text)
            .map_err(|error| Error::Backend(format!("{} is damaged: {error}", path.display())))?;
        if envelope.version != FORMAT_VERSION {
            return Err(Error::Backend(format!(
                "{} has format version {}, this app reads version {FORMAT_VERSION}",
                path.display(),
                envelope.version
            )));
        }

        let salt: [u8; SALT_LENGTH] = decode_exact(&envelope.salt, path)?;
        let nonce: [u8; NONCE_LENGTH] = decode_exact(&envelope.nonce, path)?;
        let ciphertext = hex::decode(&envelope.ciphertext).map_err(|_| damaged(path))?;
        let cost = Cost {
            memory_kib: envelope.memory_kib,
            iterations: envelope.iterations,
            parallelism: envelope.parallelism,
        };
        if cost.memory_kib > MAX_MEMORY_KIB || cost.iterations > MAX_ITERATIONS {
            return Err(damaged(path));
        }

        let key = derive_key(master_password, &salt, cost)?;
        let payload = Payload {
            msg: &ciphertext,
            aad: ASSOCIATED_DATA,
        };
        // A wrong password and a tampered file fail the same way, by design of the cipher.
        let plaintext = cipher(&key)
            .decrypt(&XNonce::from(nonce), payload)
            .map(Zeroizing::new)
            .map_err(|_| Error::WrongPassword)?;

        let text = std::str::from_utf8(&plaintext).map_err(|_| damaged(path))?;
        let stored: BTreeMap<String, String> = toml::from_str(text).map_err(|_| damaged(path))?;
        let secrets = stored
            .into_iter()
            .map(|(id, secret)| (id, Secret::new(secret)))
            .collect();

        Ok(FileStore {
            path: path.to_path_buf(),
            key,
            salt,
            cost,
            secrets: Mutex::new(secrets),
        })
    }

    fn secrets(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Secret>> {
        // The map is only replaced after a successful write, so a poisoned lock is still consistent.
        self.secrets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Encrypts the whole map under a new nonce and replaces the file in one rename.
    fn write(&self, secrets: &BTreeMap<String, Secret>) -> Result<()> {
        let plain: BTreeMap<&str, &str> = secrets
            .iter()
            .map(|(id, secret)| (id.as_str(), secret.expose()))
            .collect();
        let plaintext = Zeroizing::new(
            toml::to_string(&plain).map_err(|error| Error::Backend(error.to_string()))?,
        );

        let mut nonce = [0u8; NONCE_LENGTH];
        fill_random(&mut nonce)?;
        let payload = Payload {
            msg: plaintext.as_bytes(),
            aad: ASSOCIATED_DATA,
        };
        let ciphertext = cipher(&self.key)
            .encrypt(&XNonce::from(nonce), payload)
            .map_err(|_| Error::Backend("encryption failed".to_string()))?;

        let envelope = Envelope {
            version: FORMAT_VERSION,
            memory_kib: self.cost.memory_kib,
            iterations: self.cost.iterations,
            parallelism: self.cost.parallelism,
            salt: hex::encode(self.salt),
            nonce: hex::encode(nonce),
            ciphertext: hex::encode(ciphertext),
        };
        let text = toml::to_string(&envelope).map_err(|error| Error::Backend(error.to_string()))?;

        write_atomically(&self.path, text.as_bytes()).map_err(|error| io_error(&self.path, error))
    }
}

impl SecretStore for FileStore {
    fn get(&self, credential_id: &str) -> Result<Option<Secret>> {
        Ok(self.secrets().get(credential_id).cloned())
    }

    fn set(&self, credential_id: &str, secret: &Secret) -> Result<()> {
        let mut secrets = self.secrets();
        let mut updated = secrets.clone();
        updated.insert(credential_id.to_string(), secret.clone());
        // Memory changes only once the file has: the two never disagree.
        self.write(&updated)?;
        *secrets = updated;
        Ok(())
    }

    fn delete(&self, credential_id: &str) -> Result<()> {
        let mut secrets = self.secrets();
        if !secrets.contains_key(credential_id) {
            return Ok(());
        }
        let mut updated = secrets.clone();
        updated.remove(credential_id);
        self.write(&updated)?;
        *secrets = updated;
        Ok(())
    }
}

fn derive_key(master_password: &Secret, salt: &[u8], cost: Cost) -> Result<Key> {
    let params = Params::new(
        cost.memory_kib,
        cost.iterations,
        cost.parallelism,
        Some(KEY_LENGTH),
    )
    .map_err(|error| Error::Backend(format!("bad key-derivation settings: {error}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; KEY_LENGTH]);
    argon2
        .hash_password_into(
            master_password.expose().as_bytes(),
            salt,
            key.as_mut_slice(),
        )
        .map_err(|error| Error::Backend(format!("key derivation failed: {error}")))?;
    Ok(key)
}

fn cipher(key: &Key) -> XChaCha20Poly1305 {
    let bytes: &[u8; KEY_LENGTH] = key;
    XChaCha20Poly1305::new(bytes.into())
}

fn fill_random(buffer: &mut [u8]) -> Result<()> {
    getrandom::fill(buffer)
        .map_err(|error| Error::Backend(format!("no random numbers available: {error}")))
}

fn decode_exact<const LENGTH: usize>(text: &str, path: &Path) -> Result<[u8; LENGTH]> {
    let bytes = hex::decode(text).map_err(|_| damaged(path))?;
    bytes.try_into().map_err(|_| damaged(path))
}

fn damaged(path: &Path) -> Error {
    Error::Backend(format!("{} is damaged", path.display()))
}

fn io_error(path: &Path, error: std::io::Error) -> Error {
    Error::Backend(format!("cannot access {}: {error}", path.display()))
}

/// Writes beside the target, flushes to disk, then renames over it.
fn write_atomically(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = Path::new(&temporary);

    let mut file = create_private(temporary)?;
    file.write_all(content)?;
    file.sync_all()?;
    fs::rename(temporary, path)
}

/// On Unix the file is readable by its owner only, from the moment it exists.
#[cfg(unix)]
fn create_private(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;

    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private(path: &Path) -> std::io::Result<File> {
    File::create(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(text: &str) -> Secret {
        Secret::new(text.to_string())
    }

    fn text(secret: Option<Secret>) -> Option<String> {
        secret.map(|secret| secret.expose().to_string())
    }

    fn store_path(directory: &tempfile::TempDir) -> PathBuf {
        directory.path().join("secrets.toml")
    }

    #[test]
    fn secrets_survive_closing_and_reopening() {
        let directory = tempfile::tempdir().unwrap();
        let path = store_path(&directory);
        let store = FileStore::create(&path, &secret("master")).unwrap();
        store.set("cred-ops", &secret("hunter2")).unwrap();
        store
            .set("cred-dev", &secret("pässwörd \"quoted\"\nline two"))
            .unwrap();
        drop(store);

        let reopened = FileStore::open(&path, &secret("master")).unwrap();

        assert_eq!(
            text(reopened.get("cred-ops").unwrap()).as_deref(),
            Some("hunter2")
        );
        assert_eq!(
            text(reopened.get("cred-dev").unwrap()).as_deref(),
            Some("pässwörd \"quoted\"\nline two")
        );
        assert_eq!(text(reopened.get("cred-none").unwrap()), None);
    }

    #[test]
    fn a_wrong_master_password_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let path = store_path(&directory);
        FileStore::create(&path, &secret("master")).unwrap();

        let result = FileStore::open(&path, &secret("not the master"));

        assert!(matches!(result, Err(Error::WrongPassword)));
    }

    #[test]
    fn neither_secrets_nor_ids_appear_in_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = store_path(&directory);
        let store = FileStore::create(&path, &secret("master")).unwrap();
        store.set("cred-ops", &secret("hunter2")).unwrap();

        let on_disk = fs::read_to_string(&path).unwrap();

        assert!(!on_disk.contains("hunter2"));
        assert!(!on_disk.contains(&hex::encode("hunter2")));
        assert!(!on_disk.contains("cred-ops"));
        assert!(!on_disk.contains("master"));
    }

    #[test]
    fn a_tampered_file_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let path = store_path(&directory);
        let store = FileStore::create(&path, &secret("master")).unwrap();
        store.set("cred-ops", &secret("hunter2")).unwrap();

        let mut envelope: Envelope = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let mut bytes = hex::decode(&envelope.ciphertext).unwrap();
        bytes[0] ^= 1;
        envelope.ciphertext = hex::encode(bytes);
        fs::write(&path, toml::to_string(&envelope).unwrap()).unwrap();

        assert!(matches!(
            FileStore::open(&path, &secret("master")),
            Err(Error::WrongPassword)
        ));
    }

    #[test]
    fn every_write_uses_a_new_nonce() {
        let directory = tempfile::tempdir().unwrap();
        let path = store_path(&directory);
        let store = FileStore::create(&path, &secret("master")).unwrap();
        let nonce_on_disk = || {
            let envelope: Envelope = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            envelope.nonce
        };

        let first = nonce_on_disk();
        store.set("cred-ops", &secret("hunter2")).unwrap();
        let second = nonce_on_disk();
        store.set("cred-ops", &secret("hunter2")).unwrap();
        let third = nonce_on_disk();

        assert_ne!(first, second);
        assert_ne!(second, third);
    }

    #[test]
    fn two_stores_with_the_same_password_use_different_salts() {
        let directory = tempfile::tempdir().unwrap();
        let first = FileStore::create(&directory.path().join("a.toml"), &secret("master")).unwrap();
        let second =
            FileStore::create(&directory.path().join("b.toml"), &secret("master")).unwrap();

        assert_ne!(first.salt, second.salt);
        assert_ne!(*first.key, *second.key);
    }

    #[test]
    fn a_deleted_secret_is_gone_after_reopening() {
        let directory = tempfile::tempdir().unwrap();
        let path = store_path(&directory);
        let store = FileStore::create(&path, &secret("master")).unwrap();
        store.set("cred-ops", &secret("hunter2")).unwrap();

        store.delete("cred-ops").unwrap();
        store.delete("cred-ops").unwrap();
        drop(store);

        let reopened = FileStore::open(&path, &secret("master")).unwrap();
        assert_eq!(text(reopened.get("cred-ops").unwrap()), None);
    }

    #[test]
    fn create_does_not_overwrite_an_existing_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = store_path(&directory);
        let store = FileStore::create(&path, &secret("master")).unwrap();
        store.set("cred-ops", &secret("hunter2")).unwrap();

        assert!(FileStore::create(&path, &secret("other")).is_err());

        let reopened = FileStore::open(&path, &secret("master")).unwrap();
        assert!(reopened.get("cred-ops").unwrap().is_some());
    }

    #[test]
    fn a_file_that_is_not_a_store_is_reported_as_damaged() {
        let directory = tempfile::tempdir().unwrap();
        let path = store_path(&directory);
        fs::write(&path, "this is not a secret store").unwrap();

        let result = FileStore::open(&path, &secret("master"));

        assert!(matches!(result, Err(Error::Backend(reason)) if reason.contains("damaged")));
    }

    #[test]
    fn absurd_key_derivation_settings_are_refused_before_any_work() {
        let directory = tempfile::tempdir().unwrap();
        let path = store_path(&directory);
        FileStore::create(&path, &secret("master")).unwrap();

        let mut envelope: Envelope = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        envelope.memory_kib = u32::MAX;
        fs::write(&path, toml::to_string(&envelope).unwrap()).unwrap();

        let result = FileStore::open(&path, &secret("master"));

        assert!(matches!(result, Err(Error::Backend(reason)) if reason.contains("damaged")));
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_readable_by_its_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().unwrap();
        let path = store_path(&directory);
        let store = FileStore::create(&path, &secret("master")).unwrap();
        store.set("cred-ops", &secret("hunter2")).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode();

        assert_eq!(mode & 0o777, 0o600);
    }
}
