//! The host keys the user has accepted, kept in our own file.
//!
//! A key is compared as the text a server offers it in (`ssh-ed25519 AAAA...`),
//! so this module needs to know nothing about key formats.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::file_store::write_atomically;
use crate::{Error, Result};

/// What the store says about a key a server offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKeyVerdict {
    /// The same key was accepted before.
    Trusted,
    /// Nothing is stored for this host and port.
    Unknown,
    /// A different key is stored: the server was reinstalled, or someone is in the middle.
    Changed,
}

/// Accepted host keys, by host and port.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownHosts {
    path: PathBuf,
    keys: BTreeMap<String, String>,
}

/// The file's shape: one `"host:port" = "key"` line under `[hosts]`.
#[derive(Default, Serialize, Deserialize)]
struct FileContent {
    #[serde(default)]
    hosts: BTreeMap<String, String>,
}

impl KnownHosts {
    /// Reads the file. A missing file is an empty store.
    pub fn load(path: &Path) -> Result<Self> {
        let keys = match std::fs::read_to_string(path) {
            Ok(text) => {
                let content: FileContent =
                    toml::from_str(&text).map_err(|error| Error::Backend(error.to_string()))?;
                content.hosts
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(Error::Backend(error.to_string())),
        };
        Ok(Self {
            path: path.to_path_buf(),
            keys,
        })
    }

    /// Compares an offered key with the stored one.
    pub fn check(&self, host: &str, port: u16, key: &str) -> HostKeyVerdict {
        match self.keys.get(&entry_name(host, port)) {
            Some(stored) if stored == key => HostKeyVerdict::Trusted,
            Some(_) => HostKeyVerdict::Changed,
            None => HostKeyVerdict::Unknown,
        }
    }

    /// Accepts a key for a host, replacing any earlier one, and saves the file.
    pub fn trust(&mut self, host: &str, port: u16, key: &str) -> Result<()> {
        let mut keys = self.keys.clone();
        keys.insert(entry_name(host, port), key.to_string());

        let content = FileContent { hosts: keys };
        let text = toml::to_string(&content).map_err(|error| Error::Backend(error.to_string()))?;
        if let Some(directory) = self.path.parent() {
            std::fs::create_dir_all(directory)
                .map_err(|error| Error::Backend(error.to_string()))?;
        }
        write_atomically(&self.path, text.as_bytes())
            .map_err(|error| Error::Backend(error.to_string()))?;

        // Memory follows the file, never the other way round.
        self.keys = content.hosts;
        Ok(())
    }
}

/// Host names are case-insensitive; IPv6 addresses contain colons, so they get brackets.
fn entry_name(host: &str, port: u16) -> String {
    let host = host.to_lowercase();
    if host.contains(':') {
        return format!("[{host}]:{port}");
    }
    format!("{host}:{port}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFirst";
    const OTHER_KEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAISecond";

    fn store(directory: &tempfile::TempDir) -> KnownHosts {
        KnownHosts::load(&directory.path().join("known_hosts.toml")).unwrap()
    }

    #[test]
    fn a_host_never_seen_is_unknown() {
        let directory = tempfile::tempdir().unwrap();

        let verdict = store(&directory).check("web-01.example.net", 22, KEY);

        assert_eq!(verdict, HostKeyVerdict::Unknown);
    }

    #[test]
    fn a_trusted_key_is_remembered_across_loads() {
        let directory = tempfile::tempdir().unwrap();
        store(&directory)
            .trust("web-01.example.net", 22, KEY)
            .unwrap();

        let verdict = store(&directory).check("web-01.example.net", 22, KEY);

        assert_eq!(verdict, HostKeyVerdict::Trusted);
    }

    #[test]
    fn a_different_key_for_a_known_host_is_reported_as_changed() {
        let directory = tempfile::tempdir().unwrap();
        let mut hosts = store(&directory);
        hosts.trust("web-01.example.net", 22, KEY).unwrap();

        let verdict = hosts.check("web-01.example.net", 22, OTHER_KEY);

        assert_eq!(verdict, HostKeyVerdict::Changed);
    }

    #[test]
    fn the_port_is_part_of_the_identity() {
        let directory = tempfile::tempdir().unwrap();
        let mut hosts = store(&directory);
        hosts.trust("web-01.example.net", 22, KEY).unwrap();

        let verdict = hosts.check("web-01.example.net", 2222, KEY);

        assert_eq!(verdict, HostKeyVerdict::Unknown);
    }

    #[test]
    fn host_names_match_whatever_their_case() {
        let directory = tempfile::tempdir().unwrap();
        let mut hosts = store(&directory);
        hosts.trust("Web-01.Example.NET", 22, KEY).unwrap();

        let verdict = hosts.check("web-01.example.net", 22, KEY);

        assert_eq!(verdict, HostKeyVerdict::Trusted);
    }

    #[test]
    fn trusting_again_replaces_the_old_key() {
        let directory = tempfile::tempdir().unwrap();
        let mut hosts = store(&directory);
        hosts.trust("web-01.example.net", 22, KEY).unwrap();

        hosts.trust("web-01.example.net", 22, OTHER_KEY).unwrap();

        assert_eq!(
            store(&directory).check("web-01.example.net", 22, OTHER_KEY),
            HostKeyVerdict::Trusted
        );
    }

    #[test]
    fn ipv6_addresses_are_kept_apart_from_their_port() {
        assert_eq!(entry_name("::1", 22), "[::1]:22");
        assert_eq!(entry_name("10.0.0.5", 22), "10.0.0.5:22");
    }

    #[test]
    fn a_broken_file_is_an_error_not_an_empty_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("known_hosts.toml");
        std::fs::write(&path, "[hosts\n").unwrap();

        assert!(KnownHosts::load(&path).is_err());
    }
}
