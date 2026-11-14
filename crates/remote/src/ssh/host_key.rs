//! Checks the key a server offers against the accepted ones during the handshake.

use std::sync::{Arc, Mutex};

use remotething_secrets::{HostKeyVerdict, KnownHosts};
use russh::client;
use russh::keys::{HashAlg, PublicKeyOrCertificate};

use super::{Endpoint, Error, HostKey};

/// Where the handler leaves the reason for refusing a key, for [`super::open_shell`] to report.
pub(super) type Refusal = Arc<Mutex<Option<Error>>>;

/// The russh callbacks of one connection attempt.
pub(super) struct Handler {
    pub(super) endpoint: Endpoint,
    pub(super) known: KnownHosts,
    pub(super) refusal: Refusal,
}

impl client::Handler for Handler {
    type Error = russh::Error;

    /// Runs before anything secret is sent. Only an accepted key lets the handshake go on.
    async fn check_server_key(
        &mut self,
        offered: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let verdict = match describe(offered) {
            Ok(host_key) => judge(&self.known, &self.endpoint, host_key),
            Err(error) => Err(error),
        };
        match verdict {
            Ok(()) => Ok(true),
            Err(error) => {
                if let Ok(mut refusal) = self.refusal.lock() {
                    *refusal = Some(error);
                }
                Ok(false)
            }
        }
    }
}

/// The offered key as text and fingerprint.
fn describe(offered: &PublicKeyOrCertificate) -> Result<HostKey, Error> {
    let PublicKeyOrCertificate::PublicKey { key, .. } = offered else {
        let reason = "the server identified itself with a certificate, which is not supported yet";
        return Err(Error::Protocol(reason.to_string()));
    };
    let text = key
        .to_openssh()
        .map_err(|error| Error::Protocol(error.to_string()))?;
    Ok(HostKey {
        key: text,
        fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
    })
}

/// Accepts only a key the user has accepted for this host and port before.
fn judge(known: &KnownHosts, endpoint: &Endpoint, offered: HostKey) -> Result<(), Error> {
    match known.check(&endpoint.host, endpoint.port, &offered.key) {
        HostKeyVerdict::Trusted => Ok(()),
        HostKeyVerdict::Unknown => Err(Error::UnknownHostKey(offered)),
        HostKeyVerdict::Changed => Err(Error::HostKeyChanged(offered)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint() -> Endpoint {
        Endpoint {
            host: "web-01.example.net".to_string(),
            port: 22,
        }
    }

    fn host_key(key: &str) -> HostKey {
        HostKey {
            key: key.to_string(),
            fingerprint: "SHA256:test".to_string(),
        }
    }

    fn known_with(key: &str) -> (tempfile::TempDir, KnownHosts) {
        let directory = tempfile::tempdir().unwrap();
        let mut known = KnownHosts::load(&directory.path().join("known_hosts.toml")).unwrap();
        known.trust("web-01.example.net", 22, key).unwrap();
        (directory, known)
    }

    #[test]
    fn an_accepted_key_passes() {
        let (_directory, known) = known_with("ssh-ed25519 AAAA1");

        assert!(judge(&known, &endpoint(), host_key("ssh-ed25519 AAAA1")).is_ok());
    }

    #[test]
    fn a_first_contact_is_refused_and_carries_the_key_to_show() {
        let known = KnownHosts::default();

        let refused = judge(&known, &endpoint(), host_key("ssh-ed25519 AAAA1"));

        match refused {
            Err(Error::UnknownHostKey(key)) => assert_eq!(key.key, "ssh-ed25519 AAAA1"),
            other => panic!("expected an unknown host key, got {other:?}"),
        }
    }

    #[test]
    fn a_changed_key_is_refused_as_changed() {
        let (_directory, known) = known_with("ssh-ed25519 AAAA1");

        let refused = judge(&known, &endpoint(), host_key("ssh-ed25519 AAAA2"));

        assert!(matches!(refused, Err(Error::HostKeyChanged(_))));
    }
}
