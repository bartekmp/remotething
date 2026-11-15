//! Tests of `tabs.rs`.

use std::sync::Arc;

use remotething_core::{Quality, Store};
use remotething_secrets::{KnownHosts, MemoryStore, SecretStore};

use super::*;

fn connection(id: &str, protocol: Protocol) -> ResolvedConnection {
    ResolvedConnection {
        id: id.to_string(),
        name: id.to_string(),
        protocol,
        host: format!("{id}.example.net"),
        port: 22,
        credential: None,
        jump: None,
        forwards: Vec::new(),
        desktop_quality: Quality::Balanced,
    }
}

/// Everything the tabs ask the window for.
struct Fixture {
    _directory: tempfile::TempDir,
    store: Store,
    secrets: Arc<dyn SecretStore>,
    known_hosts: KnownHosts,
    recent: Recent,
    state: State,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let recent = Recent::load(&directory.path().join("state.toml"));
        Self {
            _directory: directory,
            store: Store::default(),
            secrets: Arc::new(MemoryStore::new()),
            known_hosts: KnownHosts::default(),
            recent,
            state: State::default(),
        }
    }

    /// Opens one RDP tab per id; they need no network.
    fn with_tabs(ids: &[&str]) -> Self {
        let mut fixture = Self::new();
        for id in ids {
            fixture.open(connection(id, Protocol::Rdp));
        }
        fixture
    }

    fn open(&mut self, connection: ResolvedConnection) {
        let context = Context {
            store: &self.store,
            secrets: &self.secrets,
            known_hosts: &mut self.known_hosts,
        };
        let _ = self.state.open(connection, &context, &mut self.recent);
    }

    fn send(&mut self, message: Message) {
        let mut context = Context {
            store: &self.store,
            secrets: &self.secrets,
            known_hosts: &mut self.known_hosts,
        };
        let _ = update(&mut self.state, message, &mut context, &mut self.recent);
    }

    fn titles(&self) -> Vec<&str> {
        self.state
            .tabs
            .iter()
            .map(|tab| tab.title.as_str())
            .collect()
    }

    fn active(&self) -> &str {
        self.state.tabs[self.state.active].title.as_str()
    }

    /// Turns the tab in front into one with a connected session.
    fn make_front_live(&mut self) {
        let mut session = session::State::start(
            connection("live", Protocol::Ssh),
            &Context {
                store: &self.store,
                secrets: &self.secrets,
                known_hosts: &mut self.known_hosts,
            },
        )
        .0;
        session.connect_for_test();
        let front = self.state.active;
        self.state.tabs[front].content = Content::Session(Box::new(session));
    }
}

#[test]
fn a_new_tab_comes_to_the_front_and_is_remembered_as_recent() {
    let fixture = Fixture::with_tabs(&["a", "b"]);

    assert_eq!(fixture.active(), "b");
    assert_eq!(fixture.recent.ids(), ["b", "a"]);
}

#[test]
fn pressing_a_tab_brings_it_to_the_front() {
    let mut fixture = Fixture::with_tabs(&["a", "b"]);

    fixture.send(Message::TabPressed(0));

    assert_eq!(fixture.active(), "a");
}

#[test]
fn closing_a_tab_to_the_left_keeps_the_active_one() {
    let mut fixture = Fixture::with_tabs(&["a", "b", "c"]);

    fixture.send(Message::CloseRequested(0));

    assert_eq!(fixture.active(), "c");
}

#[test]
fn closing_the_active_last_tab_moves_left() {
    let mut fixture = Fixture::with_tabs(&["a", "b", "c"]);

    fixture.send(Message::CloseRequested(2));

    assert_eq!(fixture.active(), "b");
}

#[test]
fn closing_the_only_tab_leaves_none() {
    let mut fixture = Fixture::with_tabs(&["a"]);

    fixture.send(Message::CloseRequested(0));

    assert!(fixture.state.tabs.is_empty());
}

#[test]
fn an_out_of_range_index_is_ignored() {
    let mut fixture = Fixture::with_tabs(&["a"]);

    fixture.send(Message::CloseRequested(5));
    fixture.send(Message::TabPressed(5));

    assert_eq!(fixture.titles(), ["a"]);
}

#[test]
fn a_tab_without_a_live_session_closes_without_a_question() {
    let mut fixture = Fixture::with_tabs(&["a"]);

    fixture.state.close_front();

    assert!(fixture.state.tabs.is_empty());
    assert_eq!(fixture.state.confirming_close, None);
}

#[test]
fn a_live_session_is_only_closed_after_a_yes() {
    let mut fixture = Fixture::with_tabs(&["a"]);
    fixture.make_front_live();
    assert!(fixture.state.front_is_terminal());

    fixture.state.close_front();
    assert_eq!(fixture.state.tabs.len(), 1);

    fixture.send(Message::CloseCancelled);
    assert_eq!(fixture.state.tabs.len(), 1);

    fixture.state.close_front();
    fixture.send(Message::CloseConfirmed);
    assert!(fixture.state.tabs.is_empty());
}

#[test]
fn enter_and_escape_answer_the_close_question_and_other_keys_are_swallowed() {
    let mut fixture = Fixture::with_tabs(&["a"]);
    fixture.make_front_live();

    fixture.state.close_front();
    let _ = fixture
        .state
        .key(&Key::Named(Named::Escape), Modifiers::empty(), None);
    assert_eq!(fixture.state.confirming_close, None);
    assert_eq!(fixture.state.tabs.len(), 1);

    fixture.state.close_front();
    let _ = fixture
        .state
        .key(&Key::Character("x".into()), Modifiers::empty(), Some("x"));
    assert!(fixture.state.confirming_close.is_some());
    let _ = fixture
        .state
        .key(&Key::Named(Named::Enter), Modifiers::empty(), None);
    assert!(fixture.state.tabs.is_empty());
}

#[test]
fn a_tab_dropped_on_another_takes_its_place() {
    let mut fixture = Fixture::with_tabs(&["a", "b", "c"]);

    fixture.send(Message::TabPressed(0));
    fixture.send(Message::TabEntered(2));
    fixture.send(Message::TabExited(0));
    fixture.state.release();

    assert_eq!(fixture.titles(), ["b", "c", "a"]);
    assert_eq!(fixture.active(), "a");
}

#[test]
fn a_click_on_a_tab_moves_nothing() {
    let mut fixture = Fixture::with_tabs(&["a", "b"]);

    fixture.send(Message::TabPressed(0));
    fixture.state.release();
    fixture.state.release();

    assert_eq!(fixture.titles(), ["a", "b"]);
}

#[test]
fn a_new_tab_page_turns_into_the_connection_picked_in_it() {
    let mut fixture = Fixture::new();
    fixture.store = toml::from_str(
        r#"
        [[connection]]
        id = "desk"
        name = "Desk"
        protocol = "rdp"
        host = "desk.example.net"
        "#,
    )
    .unwrap();

    let _ = fixture.state.open_launcher();
    assert!(fixture.state.open_ids().is_empty());
    let id = fixture.state.tabs[0].id;

    let typed = launcher::Message::QueryChanged("des".to_string());
    fixture.send(Message::Launcher(id, typed));
    fixture.send(Message::Launcher(id, launcher::Message::Submitted));

    assert_eq!(fixture.state.open_ids(), ["desk"]);
    assert_eq!(fixture.titles(), ["Desk"]);
    assert_eq!(fixture.recent.ids(), ["desk"]);
}

#[test]
fn a_typed_connection_string_opens_in_the_tab_without_being_saved_or_remembered() {
    let mut fixture = Fixture::new();
    let _ = fixture.state.open_launcher();
    let id = fixture.state.tabs[0].id;

    let typed = launcher::Message::QueryChanged("ssh ops@192.168.0.1".to_string());
    fixture.send(Message::Launcher(id, typed));
    fixture.send(Message::Launcher(id, launcher::Message::Submitted));

    assert_eq!(fixture.titles(), ["ops@192.168.0.1"]);
    assert!(fixture.state.tabs[0].unsaved.is_some());
    assert!(fixture.store.connections.is_empty());
    assert!(fixture.recent.ids().is_empty());
}

#[test]
fn save_in_the_menu_of_a_typed_connection_hands_it_to_the_window_once() {
    let mut fixture = Fixture::new();
    let _ = fixture.state.open_launcher();
    let id = fixture.state.tabs[0].id;
    let target = parse_quick("rdp://desk:3390").unwrap();
    let picked = launcher::Message::QuickPicked(target.clone());
    fixture.send(Message::Launcher(id, picked));

    fixture.send(Message::MenuRequested(0));
    fixture.send(Message::MenuSavePressed);

    assert_eq!(fixture.state.take_save_request(), Some((id, target)));
    assert_eq!(fixture.state.take_save_request(), None);

    fixture.state.mark_saved(id, "desk", "desk");
    assert!(fixture.state.tabs[0].unsaved.is_none());
    assert_eq!(fixture.state.open_ids(), ["desk"]);
}

#[test]
fn a_saved_connections_tab_has_nothing_to_save() {
    let mut fixture = Fixture::with_tabs(&["a"]);

    fixture.send(Message::MenuRequested(0));
    fixture.send(Message::MenuSavePressed);

    assert_eq!(fixture.state.take_save_request(), None);
}

#[test]
fn close_in_the_tab_menu_closes_that_tab_and_escape_only_closes_the_menu() {
    let mut fixture = Fixture::with_tabs(&["a", "b"]);

    fixture.send(Message::MenuRequested(0));
    let _ = fixture
        .state
        .key(&Key::Named(Named::Escape), Modifiers::empty(), None);
    assert_eq!(fixture.titles(), ["a", "b"]);

    fixture.send(Message::MenuRequested(0));
    fixture.send(Message::MenuClosePressed);
    assert_eq!(fixture.titles(), ["b"]);
}

#[test]
fn a_tab_from_the_overflow_list_comes_to_the_front_and_the_list_closes() {
    let mut fixture = Fixture::with_tabs(&["a", "b", "c"]);
    fixture.send(Message::OverflowToggled);

    fixture.send(Message::OverflowPicked(0));

    assert_eq!(fixture.active(), "a");
    assert!(!fixture.state.overflow_open);
}
