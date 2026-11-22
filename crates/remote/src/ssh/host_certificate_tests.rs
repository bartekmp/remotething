use russh::keys::ssh_key::certificate::Builder;
use russh::keys::{Algorithm, PrivateKey};

use super::*;

fn endpoint(host: &str, port: u16) -> Endpoint {
    Endpoint {
        host: host.to_string(),
        port,
    }
}

fn new_key() -> PrivateKey {
    PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// A certificate for `host_key` signed by `ca`, valid from `after` to `before`.
fn certificate(
    ca: &PrivateKey,
    host_key: &PrivateKey,
    cert_type: CertType,
    principals: &[&str],
    (after, before): (u64, u64),
) -> Certificate {
    let mut builder =
        Builder::new_with_random_nonce(&mut rand::rng(), host_key.public_key(), after, before)
            .unwrap();
    builder.serial(1).unwrap();
    builder.key_id("test").unwrap();
    builder.cert_type(cert_type).unwrap();
    if principals.is_empty() {
        builder.all_principals_valid().unwrap();
    }
    for principal in principals {
        builder.valid_principal(*principal).unwrap();
    }
    builder.sign(ca).unwrap()
}

fn trusting(ca: &PrivateKey, patterns: &str) -> Vec<Authority> {
    let key = ca.public_key().to_openssh().unwrap();
    parse(&format!("@cert-authority {patterns} {key} comment\n"))
}

fn valid_now() -> (u64, u64) {
    (now() - 3600, now() + 3600)
}

#[test]
fn only_cert_authority_lines_are_read() {
    let key = new_key().public_key().to_openssh().unwrap();
    let text = format!(
        "host.example {key}\n# comment\n@revoked host.example {key}\n@cert-authority *.example {key}\n@cert-authority broken\n"
    );

    let authorities = parse(&text);

    assert_eq!(authorities.len(), 1);
    assert!(authorities[0].covers(&endpoint("web.example", 22)));
}

#[test]
fn patterns_match_wildcards_negations_and_ports() {
    let authority = &trusting(&new_key(), "*.example.net,!db.example.net,[*.lab]:2222")[0];

    assert!(authority.covers(&endpoint("web.example.net", 22)));
    assert!(authority.covers(&endpoint("WEB.example.net", 22)));
    assert!(!authority.covers(&endpoint("db.example.net", 22)));
    assert!(!authority.covers(&endpoint("web.other.net", 22)));
    assert!(authority.covers(&endpoint("x.lab", 2222)));
    assert!(!authority.covers(&endpoint("x.lab", 22)));
}

#[test]
fn a_certificate_from_a_trusted_authority_is_accepted() {
    let ca = new_key();
    let host = new_key();
    let cert = certificate(
        &ca,
        &host,
        CertType::Host,
        &["web.example.net"],
        valid_now(),
    );

    let result = check(
        &trusting(&ca, "*.example.net"),
        &endpoint("web.example.net", 22),
        &cert,
    );

    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn a_certificate_without_principals_is_valid_for_any_covered_host() {
    let ca = new_key();
    let cert = certificate(&ca, &new_key(), CertType::Host, &[], valid_now());

    assert!(check(&trusting(&ca, "*"), &endpoint("any.host", 22), &cert).is_ok());
}

#[test]
fn a_certificate_from_another_authority_is_refused() {
    let (ca, other) = (new_key(), new_key());
    let cert = certificate(&other, &new_key(), CertType::Host, &[], valid_now());

    assert!(check(&trusting(&ca, "*"), &endpoint("web", 22), &cert).is_err());
}

#[test]
fn an_expired_certificate_is_refused() {
    let ca = new_key();
    let expired = (now() - 7200, now() - 3600);
    let cert = certificate(&ca, &new_key(), CertType::Host, &[], expired);

    assert!(check(&trusting(&ca, "*"), &endpoint("web", 22), &cert).is_err());
}

#[test]
fn a_certificate_for_another_host_is_refused() {
    let ca = new_key();
    let cert = certificate(
        &ca,
        &new_key(),
        CertType::Host,
        &["db.example.net"],
        valid_now(),
    );

    let result = check(&trusting(&ca, "*"), &endpoint("web.example.net", 22), &cert);

    assert!(result.is_err());
}

#[test]
fn a_user_certificate_is_not_a_host_certificate() {
    let ca = new_key();
    let cert = certificate(&ca, &new_key(), CertType::User, &[], valid_now());

    assert!(check(&trusting(&ca, "*"), &endpoint("web", 22), &cert).is_err());
}

#[test]
fn an_authority_that_does_not_cover_the_host_is_not_trusted_for_it() {
    let ca = new_key();
    let cert = certificate(&ca, &new_key(), CertType::Host, &[], valid_now());

    assert!(
        check(
            &trusting(&ca, "*.example.net"),
            &endpoint("web.other.org", 22),
            &cert
        )
        .is_err()
    );
}
