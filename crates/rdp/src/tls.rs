//! The TLS layer of an RDP connection and the check of the server's certificate.

use std::io::Write as _;
use std::net::TcpStream;
use std::sync::Arc;

use remotething_secrets::{HostKeyVerdict, KnownHosts};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned};
use sha2::{Digest, Sha256};

use crate::error::{Certificate, Error, Result};

pub(crate) type Stream = StreamOwned<ClientConnection, TcpStream>;

/// Prefix of what is stored, so a certificate pin never equals an SSH host key.
const PIN_PREFIX: &str = "rdp-cert-sha256:";

/// Starts TLS over `tcp` and returns the stream, the server's certificate and
/// the server's public key, which CredSSP binds to.
///
/// Any certificate is accepted here: RDP servers mostly use self-signed ones.
/// The handshake signatures are still verified against it, so the peer must hold
/// its key. The caller pins it with [`check`] before sending any credential.
pub(crate) fn upgrade(tcp: TcpStream, host: &str) -> Result<(Stream, Vec<u8>, Vec<u8>)> {
    let protocol = |error: &dyn std::fmt::Display| Error::Protocol(format!("TLS: {error}"));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(AcceptAnyCertificate(provider.clone()));

    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| protocol(&error))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    // CredSSP does not support TLS session resumption.
    config.resumption = rustls::client::Resumption::disabled();

    let server_name = ServerName::try_from(host.to_owned()).map_err(|error| protocol(&error))?;
    let client =
        ClientConnection::new(Arc::new(config), server_name).map_err(|error| protocol(&error))?;
    let mut stream = StreamOwned::new(client, tcp);
    // Flushing drives the handshake far enough to receive the certificate.
    stream.flush().map_err(|error| protocol(&error))?;

    let certificate = stream
        .conn
        .peer_certificates()
        .and_then(|certificates| certificates.first())
        .ok_or_else(|| protocol(&"the server sent no certificate"))?
        .as_ref()
        .to_vec();
    let public_key = public_key_of(&certificate)?;
    Ok((stream, certificate, public_key))
}

/// Compares the certificate with the pinned one for this host and port.
pub(crate) fn check(known: &KnownHosts, host: &str, port: u16, certificate: &[u8]) -> Result<()> {
    let pinned = describe(host, port, certificate);
    match known.check(host, port, &pinned.key) {
        HostKeyVerdict::Trusted => Ok(()),
        HostKeyVerdict::Unknown => Err(Error::UnknownCertificate(pinned)),
        HostKeyVerdict::Changed => Err(Error::CertificateChanged(pinned)),
    }
}

/// The certificate as it is pinned and shown.
pub(crate) fn describe(host: &str, port: u16, certificate: &[u8]) -> Certificate {
    let hash = Sha256::digest(certificate);
    let hex: Vec<String> = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    Certificate {
        host: host.to_string(),
        port,
        key: format!("{PIN_PREFIX}{}", hex.concat()),
        fingerprint: hex.join(":").to_uppercase(),
    }
}

fn public_key_of(certificate: &[u8]) -> Result<Vec<u8>> {
    use x509_cert::der::Decode as _;

    let protocol = |reason: String| Error::Protocol(format!("certificate: {reason}"));
    let certificate = x509_cert::Certificate::from_der(certificate)
        .map_err(|error| protocol(error.to_string()))?;
    let key = certificate
        .tbs_certificate()
        .subject_public_key_info()
        .subject_public_key
        .as_bytes()
        .ok_or_else(|| protocol("the public key is not byte aligned".to_string()))?;
    Ok(key.to_owned())
}

#[derive(Debug)]
struct AcceptAnyCertificate(Arc<CryptoProvider>);

impl ServerCertVerifier for AcceptAnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    // The certificate itself is judged by the pin, but the handshake must still
    // prove that the peer holds its private key; otherwise anyone could replay
    // the server's public certificate.
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            certificate,
            signature,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            certificate,
            signature,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> (tempfile::TempDir, KnownHosts) {
        let directory = tempfile::tempdir().unwrap();
        let known = KnownHosts::load(&directory.path().join("known_hosts.toml")).unwrap();
        (directory, known)
    }

    #[test]
    fn the_fingerprint_is_the_sha256_in_upper_case_pairs() {
        let described = describe("win", 3389, b"abc");

        assert_eq!(
            described.fingerprint,
            "BA:78:16:BF:8F:01:CF:EA:41:41:40:DE:5D:AE:22:23:B0:03:61:A3:96:17:7A:9C:B4:10:FF:61:F2:00:15:AD"
        );
        assert!(described.key.starts_with(PIN_PREFIX));
    }

    #[test]
    fn a_new_certificate_is_unknown_until_trusted_and_a_different_one_is_a_change() {
        let (_directory, mut known) = known();
        let first = describe("win", 3389, b"one");

        assert!(matches!(
            check(&known, "win", 3389, b"one"),
            Err(Error::UnknownCertificate(_))
        ));

        known.trust("win", 3389, &first.key).unwrap();
        assert!(check(&known, "win", 3389, b"one").is_ok());
        assert!(matches!(
            check(&known, "win", 3389, b"two"),
            Err(Error::CertificateChanged(_))
        ));
    }
}
