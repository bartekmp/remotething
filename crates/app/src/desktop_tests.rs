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
            local: remotething_core::LocalSettings::default(),
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

#[test]
fn totals_from_the_session_become_a_reading() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let _input = state.connect_for_test();

    fixture.send(
        &mut state,
        Message::Stats(Stats {
            received: 5000,
            frames: 20,
        }),
    );

    let Stage::Connected(live) = &state.stage else {
        panic!("not connected");
    };
    assert_eq!(live.meter.reading.unwrap().totals.received, 5000);
}

#[test]
fn the_info_button_opens_and_closes_the_details() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let _input = state.connect_for_test();
    let open = |state: &State| matches!(&state.stage, Stage::Connected(live) if live.details_open);

    assert!(!open(&state));
    fixture.send(&mut state, Message::DetailsToggled);
    assert!(open(&state));
    fixture.send(&mut state, Message::DetailsToggled);
    assert!(!open(&state));
}

#[test]
fn a_settled_area_asks_the_server_for_that_size() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test();
    let area = Size::new(1500.0, 900.0);

    fixture.send(&mut state, Message::AreaChanged(area));
    fixture.send(&mut state, Message::AreaSettled(area));

    assert_eq!(
        input.try_recv().unwrap(),
        Input::Resize {
            width: 1500,
            height: 900,
            scale: 100,
        }
    );
}

#[test]
fn an_area_that_changed_again_is_not_asked_for() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test();

    fixture.send(&mut state, Message::AreaChanged(Size::new(1500.0, 900.0)));
    fixture.send(&mut state, Message::AreaChanged(Size::new(1600.0, 900.0)));
    fixture.send(&mut state, Message::AreaSettled(Size::new(1500.0, 900.0)));

    assert!(input.try_recv().is_err());
}

#[test]
fn the_button_holds_ctrl_alt_delete_and_the_release_lets_go() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test();
    let drain = |input: &mut mpsc::Receiver<Input>| {
        let mut sent = Vec::new();
        while let Ok(one) = input.try_recv() {
            sent.push(one);
        }
        sent
    };

    fixture.send(&mut state, Message::CtrlAltDelPressed);
    let down = drain(&mut input);
    fixture.send(&mut state, Message::CtrlAltDelReleased);
    let up = drain(&mut input);

    assert_eq!(down, remotething_rdp::ctrl_alt_delete(true));
    assert_eq!(up, remotething_rdp::ctrl_alt_delete(false));
}

#[test]
fn a_new_local_clipboard_text_is_offered_once() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test();

    state.local_clipboard("hello".to_string());
    state.local_clipboard("hello".to_string());

    assert_eq!(
        input.try_recv().unwrap(),
        Input::Clipboard("hello".to_string())
    );
    assert!(input.try_recv().is_err());
}

#[test]
fn text_copied_on_the_server_is_not_sent_back() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test();

    fixture.send(&mut state, Message::Clipboard("from afar".to_string()));
    state.local_clipboard("from afar".to_string());

    assert!(input.try_recv().is_err());
}

#[test]
fn a_changed_local_text_is_offered_again() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test();

    state.local_clipboard("one".to_string());
    state.local_clipboard("two".to_string());

    assert_eq!(
        input.try_recv().unwrap(),
        Input::Clipboard("one".to_string())
    );
    assert_eq!(
        input.try_recv().unwrap(),
        Input::Clipboard("two".to_string())
    );
}

fn shape_cursor() -> remotething_rdp::Cursor {
    remotething_rdp::Cursor::Shape(remotething_rdp::CursorShape {
        width: 2,
        height: 2,
        hotspot_x: 1,
        hotspot_y: 1,
        rgba: vec![255; 16],
    })
}

#[test]
fn a_pointer_picture_from_the_server_hides_the_local_pointer() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let _input = state.connect_for_test();
    let hides =
        |state: &State| matches!(&state.stage, Stage::Connected(live) if live.cursor.hides_local());

    assert!(!hides(&state));
    fixture.send(&mut state, Message::Cursor(shape_cursor()));
    assert!(hides(&state));
    fixture.send(
        &mut state,
        Message::Cursor(remotething_rdp::Cursor::Default),
    );
    assert!(!hides(&state));
}

#[test]
fn the_pointer_is_drawn_only_while_it_is_over_the_picture() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let _input = state.connect_for_test();
    let over = |state: &State| matches!(&state.stage, Stage::Connected(live) if live.over);

    fixture.send(&mut state, Message::PointerOver(true));
    assert!(over(&state));
    fixture.send(&mut state, Message::PointerOver(false));
    assert!(!over(&state));
}

fn small_picture(pixel: u8) -> remotething_rdp::Picture {
    remotething_rdp::Picture {
        width: 1,
        height: 1,
        rgba: vec![pixel, 0, 0, 255],
    }
}

#[test]
fn a_new_local_picture_is_offered_once() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test();

    state.local_picture(small_picture(1));
    state.local_picture(small_picture(1));
    state.local_picture(small_picture(2));

    assert_eq!(
        input.try_recv().unwrap(),
        Input::ClipboardPicture(small_picture(1))
    );
    assert_eq!(
        input.try_recv().unwrap(),
        Input::ClipboardPicture(small_picture(2))
    );
    assert!(input.try_recv().is_err());
}

#[test]
fn a_picture_copied_on_the_server_is_not_sent_back() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test();

    fixture.send(&mut state, Message::ClipboardPicture(small_picture(7)));
    state.local_picture(small_picture(7));

    assert!(input.try_recv().is_err());
}

#[test]
fn text_in_between_lets_the_same_picture_be_copied_again() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let mut input = state.connect_for_test();

    state.local_picture(small_picture(1));
    state.local_clipboard("words".to_string());
    state.local_picture(small_picture(1));

    let mut sent = Vec::new();
    while let Ok(one) = input.try_recv() {
        sent.push(one);
    }
    assert_eq!(sent.len(), 3);
}

#[test]
fn a_saved_screenshot_is_announced_in_a_note() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let _input = state.connect_for_test();

    fixture.send(
        &mut state,
        Message::ScreenshotSaved(Ok("win-20260101-000000.png".to_string())),
    );

    let Stage::Connected(live) = &state.stage else {
        panic!("not connected");
    };
    let note = live.note.as_ref().and_then(Note::current).unwrap();
    assert!(note.contains("win-20260101-000000.png"), "{note}");
}

fn connected_with_password(fixture: &mut Fixture) -> State {
    let mut fixture_state = fixture.start(Some("cred"));
    let sealed = Sealed(Some(Secret::new("pw".to_string())));
    fixture.send(&mut fixture_state, Message::SecretLoaded(Ok(sealed)));
    let _input = fixture_state.connect_for_test();
    fixture_state
}

#[test]
fn a_lost_connection_is_made_again_by_itself() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    let mut state = connected_with_password(&mut fixture);

    fixture.send(&mut state, Message::ConnectionLost("gone".to_string()));

    // The tab stays as it was, with the last picture, while the new session opens.
    assert!(matches!(&state.stage, Stage::Connected(live) if live.reconnecting));
    assert_eq!(state.reconnects, 1);
}

#[test]
fn the_picture_and_the_clipboard_state_carry_over_to_the_new_session() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    let mut state = connected_with_password(&mut fixture);
    state.local_clipboard("words".to_string());
    fixture.send(&mut state, Message::AreaChanged(Size::new(1500.0, 900.0)));
    let painted = Picture::new(2, 2, vec![9; 16]);
    let serial = painted.serial();
    fixture.send(&mut state, Message::Frame(painted));
    fixture.send(&mut state, Message::ConnectionLost("gone".to_string()));

    // The new session arrives.
    let (_events_tx, events) = tokio::sync::mpsc::channel(4);
    let (input, mut typed) = tokio::sync::mpsc::channel(16);
    let session = remotething_rdp::Session {
        info: remotething_rdp::Info {
            peer: "127.0.0.1:3389".parse().unwrap(),
            codec: "test",
            security: "test",
        },
        events,
        input,
    };
    fixture.send(&mut state, Message::Opened(Ok(Opened::new(session))));

    let Stage::Connected(live) = &state.stage else {
        panic!("not connected");
    };
    assert!(!live.reconnecting);
    assert_eq!(live.picture.serial(), serial);
    assert_eq!(live.clipboard.as_deref(), Some("words"));
    // The new desktop is asked for the size of the room again.
    assert!(matches!(
        typed.try_recv().unwrap(),
        Input::Resize {
            width: 1500,
            height: 900,
            ..
        }
    ));
}

#[test]
fn after_three_tries_in_a_row_the_user_is_told() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password);
    let mut state = connected_with_password(&mut fixture);
    for _ in 0..3 {
        fixture.send(&mut state, Message::ConnectionLost("gone".to_string()));
        let _input = state.connect_for_test();
    }

    fixture.send(
        &mut state,
        Message::ConnectionLost("gone again".to_string()),
    );

    assert!(matches!(&state.stage, Stage::Ended(reason) if reason == "gone again"));
}

#[test]
fn a_session_without_a_known_password_is_not_re_made() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let _input = state.connect_for_test();

    fixture.send(&mut state, Message::ConnectionLost("gone".to_string()));

    assert!(matches!(state.stage, Stage::Ended(_)));
}
