//! Tests of `ftp_tab.rs`.

use remotething_core::{Credential, Quality, Store};
use remotething_secrets::{KnownHosts, MemoryStore, SecretStore};

use super::*;

struct Fixture {
    _directory: tempfile::TempDir,
    store: Store,
    secrets: Arc<dyn SecretStore>,
    known_hosts: KnownHosts,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let known_hosts = KnownHosts::load(&directory.path().join("known_hosts.toml")).unwrap();
        Self {
            _directory: directory,
            store: Store::default(),
            secrets: Arc::new(MemoryStore::new()),
            known_hosts,
        }
    }

    fn with_credential(mut self, kind: CredentialKind) -> Self {
        self.store.credentials.push(Credential {
            id: "cred".to_string(),
            name: "Files".to_string(),
            username: Some("ops".to_string()),
            domain: None,
            kind,
            key_file: None,
        });
        self
    }

    fn context(&mut self) -> Context<'_> {
        Context {
            store: &self.store,
            secrets: &self.secrets,
            known_hosts: &mut self.known_hosts,
        }
    }

    fn start(&mut self, protocol: Protocol, credential: Option<&str>) -> State {
        let connection = ResolvedConnection {
            id: "nas".to_string(),
            name: "nas".to_string(),
            protocol,
            host: "nas.example.net".to_string(),
            port: 21,
            credential: credential.map(str::to_string),
            jump: None,
            forwards: Vec::new(),
            desktop_quality: Quality::Balanced,
        };
        State::start(connection, None, &self.context()).0
    }

    fn send(&mut self, state: &mut State, message: Message) {
        let _ = update(state, message, &mut self.context());
    }
}

fn certificate() -> Certificate {
    Certificate {
        host: "nas.example.net".to_string(),
        port: 21,
        key: "ftps-cert-sha256:abcd".to_string(),
        fingerprint: "AB:CD".to_string(),
    }
}

#[test]
fn without_a_credential_the_user_name_and_password_are_asked() {
    let mut fixture = Fixture::new();

    let state = fixture.start(Protocol::Ftp, None);

    assert!(matches!(&state.stage, Stage::Asking(form) if form.ask_username));
}

#[test]
fn a_stored_password_goes_straight_to_connecting() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    let mut state = fixture.start(Protocol::Ftp, Some("cred"));
    assert!(matches!(state.stage, Stage::LoadingSecret));
    assert_eq!(state.username, "ops");

    let sealed = Sealed(Some(Secret::new("pw".to_string())));
    fixture.send(&mut state, Message::SecretLoaded(Ok(sealed)));

    assert!(matches!(state.stage, Stage::Connecting));
}

#[test]
fn a_ssh_credential_is_refused_with_a_hint() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::PrivateKey);

    let state = fixture.start(Protocol::Ftps, Some("cred"));

    assert!(matches!(state.stage, Stage::Ended(_)));
}

#[test]
fn a_missing_user_name_is_asked_for_again() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(Protocol::Ftp, None);

    fixture.send(&mut state, Message::Submitted);

    assert!(matches!(&state.stage, Stage::Asking(form) if form.note.is_some()));
}

#[test]
fn a_refused_login_asks_again_with_a_note() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    let mut state = fixture.start(Protocol::Ftp, Some("cred"));
    state.password = Some(Secret::new("wrong".to_string()));

    fixture.send(&mut state, Message::Opened(Err(Failure::Refused)));

    assert!(state.password.is_none());
    assert!(matches!(&state.stage, Stage::Asking(form) if form.note.is_some()));
}

#[test]
fn an_unknown_certificate_asks_and_trusting_it_remembers_it_and_connects() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    let mut state = fixture.start(Protocol::Ftps, Some("cred"));
    state.password = Some(Secret::new("pw".to_string()));
    let failure = Failure::Certificate {
        certificate: certificate(),
        changed: false,
    };
    fixture.send(&mut state, Message::Opened(Err(failure)));
    assert!(matches!(
        &state.stage,
        Stage::Certificate { changed: false, .. }
    ));

    fixture.send(&mut state, Message::TrustPressed);

    assert!(matches!(state.stage, Stage::Connecting));
    let verdict = fixture
        .known_hosts
        .check("nas.example.net", 21, "ftps-cert-sha256:abcd");
    assert_eq!(verdict, remotething_secrets::HostKeyVerdict::Trusted);
}

#[test]
fn only_ftps_counts_as_encrypted() {
    let mut fixture = Fixture::new();

    assert!(fixture.start(Protocol::Ftps, None).is_encrypted());
    assert!(!fixture.start(Protocol::Ftp, None).is_encrypted());
}

#[test]
fn another_failure_ends_the_tab_with_its_reason() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(Protocol::Ftp, None);

    fixture.send(
        &mut state,
        Message::Opened(Err(Failure::Other("no route".to_string()))),
    );

    assert!(matches!(&state.stage, Stage::Ended(reason) if reason == "no route"));
}

#[test]
fn errors_from_the_files_crate_become_the_matching_failure() {
    assert!(matches!(
        Failure::from(Error::LoginRefused),
        Failure::Refused
    ));
    assert!(matches!(
        Failure::from(Error::CertificateChanged(certificate())),
        Failure::Certificate { changed: true, .. }
    ));
    assert!(matches!(
        Failure::from(Error::UnknownCertificate(certificate())),
        Failure::Certificate { changed: false, .. }
    ));
}

#[test]
fn an_smb_tab_takes_the_domain_of_its_credential() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    fixture.store.credentials[0].domain = Some("CORP".to_string());

    let state = fixture.start(Protocol::Smb, Some("cred"));

    assert_eq!(state.domain.as_deref(), Some("CORP"));
    assert_eq!(state.username, "ops");
}

#[test]
fn an_empty_domain_is_no_domain() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    fixture.store.credentials[0].domain = Some(String::new());

    let state = fixture.start(Protocol::Smb, Some("cred"));

    assert_eq!(state.domain, None);
}

#[test]
fn an_smb_tab_signs_in_like_the_others_and_is_not_marked_encrypted_by_ftps_rules() {
    let mut fixture = Fixture::new();

    let state = fixture.start(Protocol::Smb, None);

    assert!(matches!(&state.stage, Stage::Asking(form) if form.ask_username));
    assert!(!state.is_encrypted());
}
