//! Tests of `vnc_tab.rs`.

use iced::keyboard::key::{Code, Named, NativeCode};
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
            name: "Screen".to_string(),
            username: None,
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

    fn start(&mut self, credential: Option<&str>) -> State {
        let connection = ResolvedConnection {
            id: "tv".to_string(),
            name: "tv".to_string(),
            protocol: Protocol::Vnc,
            host: "tv.example.net".to_string(),
            port: 5900,
            credential: credential.map(str::to_string),
            jump: None,
            forwards: Vec::new(),
            rdp_quality: Quality::Balanced,
        };
        State::start(connection, &self.context()).0
    }

    fn send(&mut self, state: &mut State, message: Message) {
        let _ = update(state, message, &mut self.context());
    }
}

fn info() -> Info {
    Info {
        peer: None,
        security: "test",
        encodings: "test",
    }
}

fn nothing() -> Physical {
    Physical::Unidentified(NativeCode::Unidentified)
}

#[test]
fn without_a_credential_the_connection_is_tried_with_no_password() {
    let mut fixture = Fixture::new();

    let state = fixture.start(None);

    assert!(matches!(state.stage, Stage::Connecting));
}

#[test]
fn a_password_credential_reads_the_stored_password_first() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);

    let state = fixture.start(Some("cred"));

    assert!(matches!(state.stage, Stage::LoadingSecret));
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
fn an_ask_every_time_credential_shows_the_form() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Ask);

    let state = fixture.start(Some("cred"));

    assert!(matches!(&state.stage, Stage::Asking(form) if form.note.is_none()));
}

#[test]
fn an_ssh_only_credential_ends_with_an_explanation() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Agent);

    let state = fixture.start(Some("cred"));

    assert!(matches!(state.stage, Stage::Ended(_)));
}

#[test]
fn a_server_that_wants_a_password_gets_a_form() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);

    fixture.send(&mut state, Message::Opened(Err(Failure::PasswordNeeded)));

    assert!(matches!(&state.stage, Stage::Asking(form) if form.note.is_some()));
}

#[test]
fn a_refused_password_shows_the_form_again_and_forgets_the_password() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    let mut state = fixture.start(Some("cred"));
    let sealed = Sealed(Some(Secret::new("pw".to_string())));
    fixture.send(&mut state, Message::SecretLoaded(Ok(sealed)));

    fixture.send(&mut state, Message::Opened(Err(Failure::WrongPassword)));

    assert!(matches!(&state.stage, Stage::Asking(form) if form.note.is_some()));
    assert!(state.password.is_none());
}

#[test]
fn a_typed_password_is_used_to_connect() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Ask);
    let mut state = fixture.start(Some("cred"));

    fixture.send(
        &mut state,
        Message::PasswordChanged(Hidden("pw".to_string())),
    );
    fixture.send(&mut state, Message::Submitted);

    assert!(matches!(state.stage, Stage::Connecting));
    assert!(state.password.is_some());
}

#[test]
fn another_failure_ends_the_session_with_its_reason() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);

    fixture.send(
        &mut state,
        Message::Opened(Err(Failure::Other("no route".to_string()))),
    );

    assert!(matches!(&state.stage, Stage::Ended(reason) if reason == "no route"));
}

#[test]
fn the_end_of_the_session_is_shown_with_its_reason() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let _input = state.connect_for_test(info());

    fixture.send(&mut state, Message::Closed(Some("lost".to_string())));

    assert!(matches!(&state.stage, Stage::Ended(reason) if reason == "lost"));
}

#[test]
fn pointer_input_carries_the_last_position_to_presses() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test(info());

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
fn a_key_sends_its_character_and_its_release_lets_go_of_the_same_one() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test(info());
    let key = Key::Character("a".into());
    let physical = Physical::Code(Code::KeyA);

    // Shift went up before the letter did: the release still lets go of `A`.
    state.key(&key, physical, Some("A"), true);
    state.key(&Key::Character("a".into()), physical, None, false);

    assert_eq!(
        input.try_recv().unwrap(),
        Input::Key {
            keysym: 0x41,
            pressed: true
        }
    );
    assert_eq!(
        input.try_recv().unwrap(),
        Input::Key {
            keysym: 0x41,
            pressed: false
        }
    );
}

#[test]
fn a_release_without_a_press_sends_nothing() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test(info());

    state.key(&Key::Named(Named::Enter), nothing(), None, false);

    assert!(input.try_recv().is_err());
}

#[test]
fn a_key_without_a_keysym_is_not_sent() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test(info());

    state.key(&Key::Unidentified, nothing(), None, true);

    assert!(input.try_recv().is_err());
}

#[test]
fn a_new_local_clipboard_text_is_offered_once_and_not_after_it_came_from_the_server() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test(info());

    state.local_clipboard("hello".to_string());
    state.local_clipboard("hello".to_string());
    fixture.send(&mut state, Message::Clipboard("from afar".to_string()));
    state.local_clipboard("from afar".to_string());

    assert_eq!(
        input.try_recv().unwrap(),
        Input::Clipboard("hello".to_string())
    );
    assert!(input.try_recv().is_err());
}

#[test]
fn stats_become_a_reading() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let _input = state.connect_for_test(info());

    fixture.send(
        &mut state,
        Message::Stats(Stats {
            received: 2000,
            sent: 100,
            frames: 30,
        }),
    );

    let Stage::Connected(live) = &state.stage else {
        panic!("not connected");
    };
    let totals = live.meter.reading.unwrap().totals;
    assert_eq!((totals.received, totals.sent), (2000, 100));
}

#[test]
fn an_empty_pointer_from_the_server_hides_the_local_one() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let _input = state.connect_for_test(info());

    fixture.send(&mut state, Message::Cursor(None));

    assert!(matches!(&state.stage, Stage::Connected(live) if live.cursor.hides_local()));
}
