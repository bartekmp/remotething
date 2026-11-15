//! Tests of `session.rs`.

use remotething_core::{Credential, Protocol, Quality};
use remotething_secrets::MemoryStore;
use tokio::sync::oneshot;

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

    fn with_credential(mut self, kind: CredentialKind, key_file: Option<&str>) -> Self {
        self.store.credentials.push(Credential {
            id: "cred".to_string(),
            name: "Ops".to_string(),
            username: Some("ops".to_string()),
            domain: None,
            kind,
            key_file: key_file.map(PathBuf::from),
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
            id: "web-01".to_string(),
            name: "web-01".to_string(),
            protocol: Protocol::Ssh,
            host: "web-01.example.net".to_string(),
            port: 22,
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

fn host_key() -> HostKey {
    HostKey {
        endpoint: remotething_remote::ssh::Endpoint {
            host: "web-01.example.net".to_string(),
            port: 22,
        },
        key: "ssh-ed25519 AAAATEST".to_string(),
        fingerprint: "SHA256:test".to_string(),
    }
}

fn form(state: &State) -> &Form {
    match &state.stage {
        Stage::Asking(form) => form,
        _ => panic!("expected the sign-in form"),
    }
}

#[test]
fn without_a_credential_the_user_name_and_password_are_asked() {
    let mut fixture = Fixture::new();

    let state = fixture.start(None);

    assert!(form(&state).ask_username);
    assert!(!state.asks_for_passphrase());
}

#[test]
fn a_typed_user_name_is_not_asked_for_again() {
    let mut fixture = Fixture::new();
    let connection = ResolvedConnection {
        id: String::new(),
        name: "ops@web-01".to_string(),
        protocol: Protocol::Ssh,
        host: "web-01".to_string(),
        port: 22,
        credential: None,
        jump: None,
        forwards: Vec::new(),
        rdp_quality: Quality::Balanced,
    };

    let typed = Some("ops".to_string());
    let (state, _) = State::start_as(connection, typed, &fixture.context());

    assert!(!form(&state).ask_username);
    assert_eq!(form(&state).username, "ops");
}

#[test]
fn an_ask_every_time_credential_only_asks_for_the_password() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Ask, None);

    let state = fixture.start(Some("cred"));

    assert!(!form(&state).ask_username);
}

#[test]
fn a_password_credential_first_reads_the_stored_password() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password, None);

    let mut state = fixture.start(Some("cred"));
    assert!(matches!(state.stage, Stage::LoadingSecret));

    // Nothing is stored, so it has to be typed.
    fixture.send(&mut state, Message::SecretLoaded(Ok(Sealed(None))));
    assert!(!form(&state).ask_username);
}

#[test]
fn a_stored_password_goes_straight_to_connecting() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password, None);
    let mut state = fixture.start(Some("cred"));

    let stored = Sealed(Some(Secret::new("hunter2".to_string())));
    fixture.send(&mut state, Message::SecretLoaded(Ok(stored)));

    assert!(matches!(state.stage, Stage::Connecting));
}

#[test]
fn a_key_credential_without_a_key_file_says_what_to_fix() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::PrivateKey, None);

    let state = fixture.start(Some("cred"));

    assert!(matches!(state.stage, Stage::Ended(_)));
}

#[test]
fn an_encrypted_key_asks_for_its_passphrase() {
    let mut fixture =
        Fixture::new().with_credential(CredentialKind::PrivateKey, Some("/keys/id_ed25519"));
    let mut state = fixture.start(Some("cred"));
    fixture.send(&mut state, Message::SecretLoaded(Ok(Sealed(None))));
    assert!(matches!(state.stage, Stage::Connecting));

    fixture.send(&mut state, Message::Opened(Err(Failure::KeyLocked)));

    assert!(form(&state).note.is_some());
    assert!(state.asks_for_passphrase());
}

#[test]
fn submitting_the_form_without_a_user_name_stays_on_the_form() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);

    fixture.send(&mut state, Message::Submitted);

    assert!(form(&state).note.is_some());
}

#[test]
fn submitting_the_form_connects_and_clears_the_typed_password() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    fixture.send(&mut state, Message::UsernameChanged("ops".to_string()));
    let typed = Message::SecretChanged(Hidden("hunter2".to_string()));
    fixture.send(&mut state, typed);

    fixture.send(&mut state, Message::Submitted);

    assert!(matches!(state.stage, Stage::Connecting));
}

#[test]
fn a_refused_password_is_asked_again_with_the_reason() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Ask, None);
    let mut state = fixture.start(Some("cred"));

    fixture.send(&mut state, Message::Opened(Err(Failure::Refused)));

    assert!(form(&state).note.is_some());
}

#[test]
fn an_unknown_host_key_is_shown_and_trusting_it_saves_it_and_connects() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Password, None);
    let mut state = fixture.start(Some("cred"));
    let stored = Sealed(Some(Secret::new("hunter2".to_string())));
    fixture.send(&mut state, Message::SecretLoaded(Ok(stored)));

    let failure = Failure::HostKey {
        key: host_key(),
        changed: false,
    };
    fixture.send(&mut state, Message::Opened(Err(failure)));
    assert!(matches!(state.stage, Stage::HostKey { changed: false, .. }));

    fixture.send(&mut state, Message::TrustPressed);

    assert!(matches!(state.stage, Stage::Connecting));
    let verdict = fixture
        .known_hosts
        .check("web-01.example.net", 22, &host_key().key);
    assert_eq!(verdict, remotething_secrets::HostKeyVerdict::Trusted);
}

#[test]
fn nothing_is_trusted_until_the_button_is_pressed() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Ask, None);
    let mut state = fixture.start(Some("cred"));

    let failure = Failure::HostKey {
        key: host_key(),
        changed: true,
    };
    fixture.send(&mut state, Message::Opened(Err(failure)));

    let verdict = fixture
        .known_hosts
        .check("web-01.example.net", 22, &host_key().key);
    assert_eq!(verdict, remotething_secrets::HostKeyVerdict::Unknown);
}

#[test]
fn any_other_failure_ends_the_session_with_its_reason() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);

    let failure = Failure::Other("cannot reach web-01".to_string());
    fixture.send(&mut state, Message::Opened(Err(failure)));

    match &state.stage {
        Stage::Ended(reason) => assert_eq!(reason, "cannot reach web-01"),
        _ => panic!("expected the session to end"),
    }
}

/// A session with a live terminal, and the other end of its input queue.
fn live(fixture: &mut Fixture) -> (State, tokio::sync::mpsc::Receiver<ShellInput>) {
    let mut state = fixture.start(None);
    let typed = state.connect_for_test();
    (state, typed)
}

fn screen_text(state: &State) -> String {
    match &state.stage {
        Stage::Connected(live) => live.screen.text(),
        _ => panic!("expected a live terminal"),
    }
}

#[test]
fn output_from_the_shell_lands_on_the_screen() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = live(&mut fixture);

    let output = Message::Shell(ShellEvent::Output(b"hello".to_vec()));
    fixture.send(&mut state, output);

    assert_eq!(screen_text(&state).lines().next(), Some("hello"));
}

#[test]
fn typed_keys_and_resizes_go_to_the_shell() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = live(&mut fixture);

    let _ = state.key(&Key::Character("l".into()), Modifiers::empty(), Some("l"));
    let size = TerminalSize {
        columns: 120,
        lines: 40,
    };
    fixture.send(
        &mut state,
        Message::Terminal(terminal_view::Event::Resized(size)),
    );

    assert_eq!(input.try_recv().unwrap(), ShellInput::Data(b"l".to_vec()));
    assert_eq!(input.try_recv().unwrap(), ShellInput::Resize(size));
}

#[test]
fn the_terminals_answers_are_sent_back() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = live(&mut fixture);

    // The remote program asks where the cursor is.
    let output = Message::Shell(ShellEvent::Output(b"\x1b[6n".to_vec()));
    fixture.send(&mut state, output);

    assert_eq!(
        input.try_recv().unwrap(),
        ShellInput::Data(b"\x1b[1;1R".to_vec())
    );
}

#[test]
fn the_wheel_scrolls_back_and_typing_returns_to_the_newest_lines() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = live(&mut fixture);
    let lines = "line\r\n".repeat(40).into_bytes();
    fixture.send(&mut state, Message::Shell(ShellEvent::Output(lines)));

    fixture.send(
        &mut state,
        Message::Terminal(terminal_view::Event::Scrolled(3)),
    );
    let scrolled_back = |state: &State| match &state.stage {
        Stage::Connected(live) => live.screen.scrolled_back(),
        _ => panic!("expected a live terminal"),
    };
    assert_eq!(scrolled_back(&state), 3);

    let _ = state.key(&Key::Named(Named::PageUp), Modifiers::SHIFT, None);
    assert!(scrolled_back(&state) > 3);

    let _ = state.key(&Key::Character("l".into()), Modifiers::empty(), Some("l"));
    assert_eq!(scrolled_back(&state), 0);
}

#[test]
fn dragging_over_the_terminal_selects_text_and_ctrl_shift_c_does_not_type() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = live(&mut fixture);
    let output = Message::Shell(ShellEvent::Output(b"hello world".to_vec()));
    fixture.send(&mut state, output);

    let started = terminal_view::Event::SelectionStarted {
        column: 0,
        line: 0,
        granularity: crate::terminal::Granularity::Character,
    };
    let extended = terminal_view::Event::SelectionExtended { column: 4, line: 0 };
    fixture.send(&mut state, Message::Terminal(started));
    fixture.send(&mut state, Message::Terminal(extended));
    fixture.send(
        &mut state,
        Message::Terminal(terminal_view::Event::SelectionEnded),
    );

    let selected = match &state.stage {
        Stage::Connected(live) => live.screen.selected_text(),
        _ => panic!("expected a live terminal"),
    };
    assert_eq!(selected.as_deref(), Some("hello"));

    let copy = Key::Character("C".into());
    let _ = state.key(&copy, Modifiers::CTRL | Modifiers::SHIFT, Some("C"));
    assert!(input.try_recv().is_err());
}

#[test]
fn pasted_text_is_typed_into_the_shell() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = live(&mut fixture);

    fixture.send(&mut state, Message::Pasted(Some("ls -l".to_string())));

    assert_eq!(
        input.try_recv().unwrap(),
        ShellInput::Data(b"ls -l".to_vec())
    );
}

#[test]
fn a_paste_with_a_line_break_waits_for_a_yes() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = live(&mut fixture);

    let pasted = Message::Pasted(Some("rm -rf build\nls".to_string()));
    fixture.send(&mut state, pasted);
    assert!(input.try_recv().is_err());

    // Keys answer the question instead of being typed.
    let _ = state.key(&Key::Character("x".into()), Modifiers::empty(), Some("x"));
    assert!(input.try_recv().is_err());

    fixture.send(&mut state, Message::PasteConfirmed);
    assert_eq!(
        input.try_recv().unwrap(),
        ShellInput::Data(b"rm -rf build\rls".to_vec())
    );
}

#[test]
fn a_cancelled_paste_sends_nothing() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = live(&mut fixture);
    fixture.send(&mut state, Message::Pasted(Some("a\nb".to_string())));

    let _ = state.key(&Key::Named(Named::Escape), Modifiers::empty(), None);
    fixture.send(&mut state, Message::PasteConfirmed);

    assert!(input.try_recv().is_err());
}

#[test]
fn a_program_that_asked_for_it_gets_pastes_marked() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = live(&mut fixture);
    let asked = Message::Shell(ShellEvent::Output(b"\x1b[?2004h".to_vec()));
    fixture.send(&mut state, asked);

    fixture.send(&mut state, Message::Pasted(Some("ls".to_string())));

    assert_eq!(
        input.try_recv().unwrap(),
        ShellInput::Data(b"\x1b[200~ls\x1b[201~".to_vec())
    );
}

#[test]
fn the_end_of_the_shell_ends_the_session() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = live(&mut fixture);

    fixture.send(&mut state, Message::Shell(ShellEvent::Closed(Some(0))));

    assert!(matches!(state.stage, Stage::Ended(_)));
}

fn connection_with_jump(jump: &str) -> ResolvedConnection {
    ResolvedConnection {
        id: "web-01".to_string(),
        name: "web-01".to_string(),
        protocol: Protocol::Ssh,
        host: "web-01.internal".to_string(),
        port: 22,
        credential: None,
        jump: Some(jump.to_string()),
        forwards: Vec::new(),
        rdp_quality: Quality::Balanced,
    }
}

#[test]
fn a_jump_host_without_a_usable_credential_ends_the_session_naming_it() {
    let mut fixture = Fixture::new();
    fixture.store = toml::from_str(
        r#"
        [[connection]]
        id = "bastion"
        name = "bastion"
        protocol = "ssh"
        host = "bastion.example.net"
        "#,
    )
    .unwrap();

    let (state, _) = State::start(connection_with_jump("bastion"), &fixture.context());

    match &state.stage {
        Stage::Ended(reason) => assert!(reason.contains("bastion")),
        _ => panic!("expected the session to end"),
    }
}

#[test]
fn a_jump_host_with_a_credential_first_loads_its_secrets_then_goes_on_to_the_target() {
    let mut fixture = Fixture::new();
    fixture.store = toml::from_str(
        r#"
        [[credential]]
        id = "ops"
        name = "Ops"
        username = "ops"
        kind = "password"

        [[connection]]
        id = "bastion"
        name = "bastion"
        protocol = "ssh"
        host = "bastion.example.net"
        credential = "ops"
        "#,
    )
    .unwrap();
    let (mut state, _) = State::start(connection_with_jump("bastion"), &fixture.context());
    assert!(matches!(state.stage, Stage::LoadingSecret));

    // The target itself has no credential, so once the jump hosts are ready it asks.
    fixture.send(
        &mut state,
        Message::JumpsLoaded(Ok(SealedJumps(Vec::new()))),
    );

    assert!(matches!(state.stage, Stage::Asking(_)));
}

#[test]
fn a_jump_host_whose_secret_is_missing_ends_the_session_with_the_reason() {
    let mut fixture = Fixture::new();
    let (mut state, _) = State::start(connection_with_jump("bastion"), &fixture.context());

    let failed = Message::JumpsLoaded(Err("no password stored".to_string()));
    fixture.send(&mut state, failed);

    match &state.stage {
        Stage::Ended(reason) => assert_eq!(reason, "no password stored"),
        _ => panic!("expected the session to end"),
    }
}

#[test]
fn trusting_a_key_saves_it_for_the_host_it_came_from_not_the_target() {
    let mut fixture = Fixture::new();
    let mut state = fixture.start(None);
    let jump_key = HostKey {
        endpoint: remotething_remote::ssh::Endpoint {
            host: "bastion.example.net".to_string(),
            port: 2222,
        },
        key: "ssh-ed25519 AAAAJUMP".to_string(),
        fingerprint: "SHA256:jump".to_string(),
    };
    let failure = Failure::HostKey {
        key: jump_key.clone(),
        changed: false,
    };
    fixture.send(&mut state, Message::Opened(Err(failure)));

    fixture.send(&mut state, Message::TrustPressed);

    let verdict = fixture
        .known_hosts
        .check("bastion.example.net", 2222, &jump_key.key);
    assert_eq!(verdict, remotething_secrets::HostKeyVerdict::Trusted);
    let target = fixture
        .known_hosts
        .check("web-01.example.net", 22, &jump_key.key);
    assert_eq!(target, remotething_secrets::HostKeyVerdict::Unknown);
}

#[test]
fn an_agent_credential_connects_at_once_without_asking_for_anything() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Agent, None);

    let state = fixture.start(Some("cred"));

    assert!(matches!(state.stage, Stage::Connecting));
}

#[test]
fn an_agent_credential_without_a_user_name_says_what_to_fix() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Agent, None);
    fixture.store.credentials[0].username = None;

    let state = fixture.start(Some("cred"));

    assert!(matches!(state.stage, Stage::Ended(_)));
}

#[test]
fn an_agent_that_the_server_refuses_ends_the_session_instead_of_asking_for_a_password() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Agent, None);
    let mut state = fixture.start(Some("cred"));

    fixture.send(&mut state, Message::Opened(Err(Failure::Refused)));

    assert!(matches!(state.stage, Stage::Ended(_)));
}

fn challenge(
    prompts: &[&str],
) -> (
    remotething_remote::ssh::Challenge,
    oneshot::Receiver<Vec<Secret>>,
) {
    let (reply, answers) = oneshot::channel();
    let challenge = remotething_remote::ssh::Challenge {
        name: String::new(),
        instructions: String::new(),
        prompts: prompts
            .iter()
            .map(|text| remotething_remote::ssh::Prompt {
                text: text.to_string(),
                echo: false,
            })
            .collect(),
        reply,
    };
    (challenge, answers)
}

fn asked(challenge: remotething_remote::ssh::Challenge) -> Message {
    Message::Challenged(challenge::Asked::new(challenge))
}

#[test]
fn an_interactive_credential_connects_without_asking_first() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Interactive, None);

    let state = fixture.start(Some("cred"));

    assert!(matches!(state.method, Method::Interactive));
    assert!(matches!(state.stage, Stage::Connecting));
}

#[test]
fn the_servers_questions_are_shown_and_the_answers_sent() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Interactive, None);
    let mut state = fixture.start(Some("cred"));
    let (challenge, mut answers) = challenge(&["Password: ", "Verification code: "]);

    fixture.send(&mut state, asked(challenge));
    fixture.send(
        &mut state,
        Message::AnswerChanged(0, Hidden("pw".to_string())),
    );
    fixture.send(
        &mut state,
        Message::AnswerChanged(1, Hidden("123456".to_string())),
    );
    fixture.send(&mut state, Message::AnswersSubmitted);

    let sent = answers.try_recv().unwrap();
    let texts: Vec<&str> = sent.iter().map(|secret| secret.expose()).collect();
    assert_eq!(texts, ["pw", "123456"]);
    assert!(matches!(state.stage, Stage::Connecting));
}

#[test]
fn cancelling_the_questions_drops_the_reply() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Interactive, None);
    let mut state = fixture.start(Some("cred"));
    let (challenge, mut answers) = challenge(&["Code: "]);
    fixture.send(&mut state, asked(challenge));

    fixture.send(&mut state, Message::AnswersCancelled);

    assert!(matches!(
        answers.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
    ));
}

#[test]
fn a_refused_interactive_sign_in_ends_with_a_reason() {
    let mut fixture = Fixture::new().with_credential(CredentialKind::Interactive, None);
    let mut state = fixture.start(Some("cred"));

    fixture.send(&mut state, Message::Opened(Err(Failure::Refused)));

    assert!(matches!(state.stage, Stage::Ended(_)));
}

fn live_of(state: &State) -> &Live {
    match &state.stage {
        Stage::Connected(live) => live,
        _ => panic!("expected a live terminal"),
    }
}

fn ctrl_shift_f(state: &mut State) {
    let key = Key::Character("F".into());
    let _ = state.key(&key, Modifiers::CTRL | Modifiers::SHIFT, None);
}

#[test]
fn ctrl_shift_f_opens_the_search_bar_and_types_nothing() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = live(&mut fixture);

    ctrl_shift_f(&mut state);

    assert!(live_of(&state).search.is_some());
    assert!(input.try_recv().is_err());
}

#[test]
fn searching_highlights_matches_and_next_walks_through_them() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = live(&mut fixture);
    let output = Message::Shell(ShellEvent::Output(b"cat\r\ndog\r\ncat".to_vec()));
    fixture.send(&mut state, output);
    ctrl_shift_f(&mut state);

    fixture.send(&mut state, Message::SearchChanged("CAT".to_string()));
    assert_eq!(live_of(&state).screen.found.len(), 2);
    // The newest match comes first.
    assert_eq!(live_of(&state).search.as_ref().unwrap().current, 1);

    fixture.send(&mut state, Message::SearchNext);
    assert_eq!(live_of(&state).search.as_ref().unwrap().current, 0);
    fixture.send(&mut state, Message::SearchPrevious);
    assert_eq!(live_of(&state).search.as_ref().unwrap().current, 1);
}

#[test]
fn new_output_drops_the_highlights_and_escape_closes_the_bar() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = live(&mut fixture);
    fixture.send(
        &mut state,
        Message::Shell(ShellEvent::Output(b"cat".to_vec())),
    );
    ctrl_shift_f(&mut state);
    fixture.send(&mut state, Message::SearchChanged("cat".to_string()));
    assert_eq!(live_of(&state).screen.found.len(), 1);

    fixture.send(
        &mut state,
        Message::Shell(ShellEvent::Output(b"more".to_vec())),
    );
    assert!(live_of(&state).screen.found.is_empty());

    let _ = state.key(&Key::Named(Named::Escape), Modifiers::empty(), None);
    assert!(live_of(&state).search.is_none());
}
