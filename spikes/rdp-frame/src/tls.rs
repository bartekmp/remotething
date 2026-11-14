//! TLS upgrade that accepts any certificate. Spike only: the real app pins them.

use std::io::Write as _;
use std::net::TcpStream;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned};

pub type Stream = StreamOwned<ClientConnection, TcpStream>;

/// Returns the encrypted stream and the server's public key, which CredSSP binds to.
pub fn upgrade(tcp: TcpStream, host: &str) -> Result<(Stream, Vec<u8>)> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(AcceptAnyCertificate(provider.clone()));

    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    // CredSSP does not support TLS session resumption.
    config.resumption = rustls::client::Resumption::disabled();

    let server_name = ServerName::try_from(host.to_owned())?;
    let client = ClientConnection::new(Arc::new(config), server_name)?;
    let mut stream = StreamOwned::new(client, tcp);
    // Flushing drives the handshake far enough to receive the certificate.
    stream.flush()?;

    let certificate = stream
        .conn
        .peer_certificates()
        .and_then(|certificates| certificates.first())
        .context("peer certificate is missing")?;
    let public_key = public_key_of(certificate)?;

    Ok((stream, public_key))
}

fn public_key_of(certificate: &[u8]) -> Result<Vec<u8>> {
    use x509_cert::der::Decode as _;

    let certificate = x509_cert::Certificate::from_der(certificate)?;
    let public_key = certificate
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .as_bytes()
        .context("public key is not byte aligned")?
        .to_owned();
    Ok(public_key)
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
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _certificate: &CertificateDer<'_>,
        _signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _certificate: &CertificateDer<'_>,
        _signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
