//! Tests of `desktop.rs`.

use remotething_core::{Credential, Protocol, Quality, Store};
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
            name: "Admin".to_string(),
            username: Some("admin".to_string()),
            domain: Some("CORP".to_string()),
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

    fn start(&mut self, credential: Option<&str>) -> State {
        let connection = ResolvedConnection {
            id: "win".to_string(),
            name: "win".to_string(),
            protocol: Protocol::Rdp,
            host: "win.example.net".to_string(),
            port: 3389,
            credential: credential.map(str::to_string),
            jump: None,
            forwards: Vec::new(),
            rdp_quality: Quality::Balanced,
        };
        State::start(connection, None, &self.context()).0
    }

    fn send(&mut self, state: &mut State, message: Message) {
        let _ = update(state, message, &mut self.context());
    }
}

fn certificate() -> Certificate {
    Certificate {
        host: "win.example.net".to_string(),
        port: 3389,
        key: "rdp-cert-sha256:abcd".to_string(),
        fingerprint: "AB:CD".to_string(),
    }
}

#[test]
fn without_a_credential_the_user_name_and_password_are_asked() {
    let mut fixture = Fixture::new();

    let state = fixture.start(None);

    assert!(matches!(&state.stage, Stage::Asking(form) if form.ask_username));
}

#[test]
fn a_password_credential_reads_the_stored_password_first() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);

    let state = fixture.start(Some("cred"));

    assert!(matches!(state.stage, Stage::LoadingSecret));
    assert_eq!(state.username, "admin");
    assert_eq!(state.domain.as_deref(), Some("CORP"));
}

#[test]
fn a_stored_password_goes_straight_to_connecting() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    let mut state = fixture.start(Some("cred"));

    let sealed = Sealed(Some(Secret::new("pw".to_string())));
    fixture.send(&mut state, Message::SecretLoaded(Ok(sealed)));

    assert!(matches!(state.stage, Stage::Connecting));
}

#[test]
fn a_password_credential_with_nothing_stored_asks_for_it() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    let mut state = fixture.start(Some("cred"));

    fixture.send(&mut state, Message::SecretLoaded(Ok(Sealed(None))));

    assert!(matches!(&state.stage, Stage::Asking(form) if !form.ask_username));
}

#[test]
fn an_ssh_only_credential_ends_with_an_explanation() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Agent);

    let state = fixture.start(Some("cred"));

    assert!(matches!(state.stage, Stage::Ended(_)));
}

#[test]
fn submitting_the_form_connects_and_clears_the_typed_password() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    fixture.send(&mut state, Message::UsernameChanged("ops".to_string()));
    fixture.send(
        &mut state,
        Message::PasswordChanged(Hidden("pw".to_string())),
    );

    fixture.send(&mut state, Message::Submitted);

    assert!(matches!(state.stage, Stage::Connecting));
    assert_eq!(state.username, "ops");
    assert!(state.password.is_some());
}

#[test]
fn submitting_without_a_user_name_stays_on_the_form() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);

    fixture.send(&mut state, Message::Submitted);

    assert!(matches!(&state.stage, Stage::Asking(form) if form.note.is_some()));
}

#[test]
fn an_unknown_certificate_is_shown_and_trusting_it_pins_it_and_connects() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    state.username = "ops".to_string();
    state.password = Some(Secret::new("pw".to_string()));
    let failure = Failure::Certificate {
        certificate: certificate(),
        changed: false,
    };

    fixture.send(&mut state, Message::Opened(Err(failure)));
    assert!(matches!(
        state.stage,
        Stage::Certificate { changed: false, .. }
    ));
    fixture.send(&mut state, Message::TrustPressed);

    assert!(matches!(state.stage, Stage::Connecting));
    let verdict = fixture
        .known_hosts
        .check("win.example.net", 3389, "rdp-cert-sha256:abcd");
    assert_eq!(verdict, remotething_secrets::HostKeyVerdict::Trusted);
}

#[test]
fn a_refused_password_comes_back_to_the_form() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);

    fixture.send(&mut state, Message::Opened(Err(Failure::Refused)));

    assert!(matches!(&state.stage, Stage::Asking(form) if form.note.is_some()));
}

#[test]
fn the_end_of_the_session_is_shown_with_its_reason() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let _input = state.connect_for_test();

    fixture.send(&mut state, Message::Closed(Some("lost".to_string())));

    assert!(matches!(&state.stage, Stage::Ended(reason) if reason == "lost"));
}

#[test]
fn pointer_input_carries_the_last_position_to_presses() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test();

    fixture.send(&mut state, Message::Moved(30, 40));
    fixture.send(&mut state, Message::Button(Button::Left, true));
    fixture.send(&mut state, Message::Wheel(120));

    assert_eq!(
        input.try_recv().unwrap(),
        Input::MouseMoved { x: 30, y: 40 }
    );
    assert_eq!(
        input.try_recv().unwrap(),
        Input::Button {
            button: Button::Left,
            pressed: true,
            x: 30,
            y: 40
        }
    );
    assert_eq!(
        input.try_recv().unwrap(),
        Input::Wheel {
            delta: 120,
            x: 30,
            y: 40
        }
    );
}

#[test]
fn keys_are_sent_as_scancodes_and_only_while_connected() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let scancode = Scancode {
        code: 0x1E,
        extended: false,
    };

    // Not connected: nothing to send to, and nothing breaks.
    state.key(scancode, true);
    let mut input = state.connect_for_test();
    state.key(scancode, true);
    state.key(scancode, false);

    assert_eq!(
        input.try_recv().unwrap(),
        Input::Key {
            code: 0x1E,
            extended: false,
            pressed: true
        }
    );
    assert!(matches!(
        input.try_recv().unwrap(),
        Input::Key { pressed: false, .. }
    ));
}
