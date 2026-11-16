//! The TLS of FTPS: any certificate is let through only if it is the one pinned
//! for the host, the same way SSH host keys and RDP certificates are pinned.

use std::sync::{Arc, Mutex};

use remotething_secrets::{HostKeyVerdict, KnownHosts};
use sha2::{Digest, Sha256};
use suppaftp::tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use suppaftp::tokio_rustls::rustls::crypto::{self, CryptoProvider};
use suppaftp::tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use suppaftp::tokio_rustls::rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};

use crate::Error;

/// Prefix of what is stored, so a certificate pin never equals an SSH host key.
const PIN_PREFIX: &str = "ftps-cert-sha256:";

/// A certificate as it is pinned and shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    /// The host the certificate was presented by.
    pub host: String,
    /// The port of that host.
    pub port: u16,
    /// What is stored in the known hosts file.
    pub key: String,
    /// The SHA-256 fingerprint in upper-case pairs, for the user to compare.
    pub fingerprint: String,
}

fn describe(host: &str, port: u16, certificate: &[u8]) -> Certificate {
    let hash = Sha256::digest(certificate);
    let hex: Vec<String> = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    Certificate {
        host: host.to_string(),
        port,
        key: format!("{PIN_PREFIX}{}", hex.concat()),
        fingerprint: hex.join(":").to_uppercase(),
    }
}

/// What the check found out, for the caller to turn into an error.
pub(crate) type Finding = Arc<Mutex<Option<Error>>>;

/// A TLS configuration that only trusts the certificate pinned for `host`:`port`.
/// When it does not, the handshake fails and `finding` says why.
pub(crate) fn pinned_config(
    known: KnownHosts,
    host: &str,
    port: u16,
    finding: Finding,
) -> ClientConfig {
    let provider = Arc::new(crypto::ring::default_provider());
    let verifier = Arc::new(PinnedCertificate {
        provider: Arc::clone(&provider),
        known,
        host: host.to_string(),
        port,
        finding,
    });
    ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("the ring provider supports the default protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth()
}

#[derive(Debug)]
struct PinnedCertificate {
    provider: Arc<CryptoProvider>,
    known: KnownHosts,
    host: String,
    port: u16,
    finding: Finding,
}

impl ServerCertVerifier for PinnedCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, suppaftp::tokio_rustls::rustls::Error> {
        let pinned = describe(&self.host, self.port, end_entity.as_ref());
        let problem = match self.known.check(&self.host, self.port, &pinned.key) {
            HostKeyVerdict::Trusted => return Ok(ServerCertVerified::assertion()),
            HostKeyVerdict::Unknown => Error::UnknownCertificate(pinned),
            HostKeyVerdict::Changed => Error::CertificateChanged(pinned),
        };
        if let Ok(mut finding) = self.finding.lock() {
            *finding = Some(problem);
        }
        Err(suppaftp::tokio_rustls::rustls::Error::General(
            "the certificate is not trusted".to_string(),
        ))
    }

    // The pin judges the certificate, but the handshake must still prove that the
    // peer holds its private key; otherwise anyone could replay the public certificate.
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, suppaftp::tokio_rustls::rustls::Error> {
        crypto::verify_tls12_signature(
            message,
            certificate,
            signature,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, suppaftp::tokio_rustls::rustls::Error> {
        crypto::verify_tls13_signature(
            message,
            certificate,
            signature,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fingerprint_is_the_sha256_in_upper_case_pairs() {
        let described = describe("ftp", 21, b"abc");

        assert_eq!(
            described.fingerprint,
            "BA:78:16:BF:8F:01:CF:EA:41:41:40:DE:5D:AE:22:23:B0:03:61:A3:96:17:7A:9C:B4:10:FF:61:F2:00:15:AD"
        );
        assert!(described.key.starts_with(PIN_PREFIX));
    }

    #[test]
    fn a_pin_never_equals_an_rdp_pin() {
        assert_ne!(PIN_PREFIX, "rdp-cert-sha256:");
    }
}
