//! What the tests against a real Windows host share.

use remotething_rdp::{Error, Quality, Target, connect};
use remotething_secrets::{KnownHosts, Secret};

pub fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

pub fn target(password: &str) -> Target {
    Target {
        host: env("RT_RDP_HOST"),
        port: 3389,
        username: env("RT_RDP_USER"),
        password: Secret::new(password.to_string()),
        domain: None,
        width: 1280,
        height: 800,
        quality: Quality::default(),
        scale: 100,
    }
}

/// A known-hosts file that already trusts the host's certificate.
pub fn trusting() -> (tempfile::TempDir, KnownHosts) {
    let directory = tempfile::tempdir().unwrap();
    let mut known = KnownHosts::load(&directory.path().join("known_hosts.toml")).unwrap();
    let Err(Error::UnknownCertificate(certificate)) =
        connect(target(&env("RT_RDP_PASSWORD")), &known)
    else {
        panic!("a first connection should stop at the certificate");
    };
    known
        .trust(&certificate.host, certificate.port, &certificate.key)
        .unwrap();
    (directory, known)
}
