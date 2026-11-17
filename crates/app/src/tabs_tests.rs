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
    fixture.state.release(false);

    assert_eq!(fixture.titles(), ["b", "c", "a"]);
    assert_eq!(fixture.active(), "a");
}

#[test]
fn a_click_on_a_tab_moves_nothing() {
    let mut fixture = Fixture::with_tabs(&["a", "b"]);

    fixture.send(Message::TabPressed(0));
    fixture.state.release(false);
    fixture.state.release(false);

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

fn dropped() -> std::path::PathBuf {
    std::path::PathBuf::from("/tmp/report.pdf")
}

#[test]
fn a_session_without_its_file_pane_open_takes_no_dropped_files() {
    let mut fixture = Fixture::with_tabs(&["a"]);
    assert!(fixture.state.file_dropped(dropped()).is_none());

    fixture.make_front_live();

    assert!(fixture.state.file_dropped(dropped()).is_none());
}

#[test]
fn a_session_with_its_file_pane_open_takes_dropped_files() {
    let mut fixture = Fixture::with_tabs(&["a"]);
    fixture.make_front_live();
    let front = fixture.state.active;
    let id = fixture.state.tabs[front].id;
    let Content::Session(session) = &mut fixture.state.tabs[front].content else {
        panic!("not a session");
    };
    session.open_files_for_test();

    let message = fixture.state.file_dropped(dropped());

    assert!(matches!(message, Some(Message::Session(tab, _)) if tab == id));
}

#[test]
fn a_file_browser_tab_takes_dropped_files_and_asks_before_it_is_closed() {
    let mut fixture = Fixture::with_tabs(&["a"]);
    fixture.make_front_live();
    let front = fixture.state.active;
    let id = fixture.state.tabs[front].id;
    let Content::Session(session) = &mut fixture.state.tabs[front].content else {
        panic!("not a session");
    };
    session.browse_for_test();

    let message = fixture.state.file_dropped(dropped());

    assert!(matches!(message, Some(Message::Session(tab, _)) if tab == id));
    fixture.state.close_front();
    assert!(fixture.state.confirming_close.is_some());
}

#[test]
fn an_ftp_tab_takes_dropped_files_once_it_shows_files_and_asks_before_it_is_closed() {
    let mut fixture = Fixture::with_tabs(&["a"]);
    let (mut ftp, _task) = ftp_tab::State::start(
        connection("files", Protocol::Ftp),
        None,
        &Context {
            store: &fixture.store,
            secrets: &fixture.secrets,
            known_hosts: &mut fixture.known_hosts,
        },
    );
    let front = fixture.state.active;
    let id = fixture.state.tabs[front].id;
    fixture.state.tabs[front].content = Content::Ftp(Box::new({
        assert!(!ftp.is_connected());
        ftp.browse_for_test();
        ftp
    }));

    let message = fixture.state.file_dropped(dropped());

    assert!(matches!(message, Some(Message::Ftp(tab, _)) if tab == id));
    fixture.state.close_front();
    assert!(fixture.state.confirming_close.is_some());
}

#[test]
fn closing_a_tab_by_request_closes_an_idle_tab_without_asking() {
    let mut fixture = Fixture::with_tabs(&["a", "b"]);

    // What a middle click on the first tab sends.
    fixture.send(Message::CloseRequested(0));

    assert_eq!(fixture.state.open_ids(), ["b"]);
}

#[test]
fn the_close_question_takes_the_keys_until_it_is_answered() {
    let mut fixture = Fixture::with_tabs(&["a"]);
    fixture.make_front_live();
    assert!(!fixture.state.has_question());

    fixture.state.close_front();
    assert!(fixture.state.has_question());

    let _ = fixture
        .state
        .key(&Key::Named(Named::Escape), Modifiers::empty(), None);
    assert!(!fixture.state.has_question());
}

#[test]
fn cycling_wraps_round_the_ends_in_both_directions() {
    let mut fixture = Fixture::with_tabs(&["a", "b", "c"]);
    assert_eq!(fixture.active(), "c");

    fixture.state.cycle(true);
    assert_eq!(fixture.active(), "a");
    fixture.state.cycle(false);
    assert_eq!(fixture.active(), "c");
}

#[test]
fn cycling_with_one_tab_or_none_changes_nothing() {
    let mut fixture = Fixture::with_tabs(&["a"]);
    fixture.state.cycle(true);
    assert_eq!(fixture.active(), "a");

    let mut empty = Fixture::new();
    empty.state.cycle(true);
    assert_eq!(empty.state.active, 0);
}

#[test]
fn a_tab_number_picks_that_tab_and_nine_is_the_last() {
    let mut fixture = Fixture::with_tabs(&["a", "b", "c"]);

    fixture.state.activate_number(1);
    assert_eq!(fixture.active(), "a");
    fixture.state.activate_number(2);
    assert_eq!(fixture.active(), "b");
    fixture.state.activate_number(9);
    assert_eq!(fixture.active(), "c");
    // A number past the tabs there are leaves the front tab where it is.
    fixture.state.activate_number(1);
    fixture.state.activate_number(5);
    assert_eq!(fixture.active(), "a");
}

fn beside(fixture: &Fixture) -> Option<&str> {
    let index = fixture.state.split_index()?;
    Some(fixture.state.tabs[index].title.as_str())
}

#[test]
fn the_menu_of_another_tab_puts_it_beside_the_front_tab() {
    let mut fixture = Fixture::with_tabs(&["a", "b", "c"]);

    fixture.send(Message::MenuRequested(0));
    fixture.send(Message::MenuSplitPressed);

    assert_eq!((fixture.active(), beside(&fixture)), ("c", Some("a")));
    assert!(fixture.state.is_shown(0) && fixture.state.is_shown(2) && !fixture.state.is_shown(1));
    assert!(fixture.state.menu.is_none());
}

#[test]
fn the_front_tab_cannot_be_split_from_itself() {
    let mut fixture = Fixture::with_tabs(&["a", "b"]);

    fixture.send(Message::MenuRequested(1));
    fixture.send(Message::MenuSplitPressed);

    assert_eq!(beside(&fixture), None);
}

#[test]
fn clicking_the_other_pane_or_a_shown_tab_trades_places_so_no_tab_is_shown_twice() {
    let mut fixture = Fixture::with_tabs(&["a", "b", "c"]);
    fixture.send(Message::MenuRequested(0));
    fixture.send(Message::MenuSplitPressed);
    let a = fixture.state.tabs[0].id;

    fixture.send(Message::PaneFocused(a));
    assert_eq!((fixture.active(), beside(&fixture)), ("a", Some("c")));

    // The bar's tab that is beside the front one: pressing it swaps too.
    fixture.send(Message::TabPressed(2));
    assert_eq!((fixture.active(), beside(&fixture)), ("c", Some("a")));

    // Another tab replaces the front one; the pair's other half stays.
    fixture.send(Message::TabPressed(1));
    assert_eq!((fixture.active(), beside(&fixture)), ("b", Some("a")));
}

#[test]
fn cycling_onto_the_tab_beside_swaps_instead_of_showing_it_twice() {
    let mut fixture = Fixture::with_tabs(&["a", "b", "c"]);
    fixture.send(Message::MenuRequested(0));
    fixture.send(Message::MenuSplitPressed);

    fixture.state.cycle(true);

    assert_eq!((fixture.active(), beside(&fixture)), ("a", Some("c")));
}

#[test]
fn closing_either_tab_of_a_split_leaves_one_tab_in_front() {
    let mut fixture = Fixture::with_tabs(&["a", "b", "c"]);
    fixture.send(Message::MenuRequested(0));
    fixture.send(Message::MenuSplitPressed);

    // Close the one beside: the front stays.
    fixture.send(Message::CloseRequested(0));
    assert_eq!((fixture.active(), beside(&fixture)), ("c", None));

    // Split again and close the front: the one beside moves up.
    fixture.send(Message::MenuRequested(0));
    fixture.send(Message::MenuSplitPressed);
    fixture.send(Message::CloseRequested(fixture.state.active));
    assert_eq!((fixture.active(), beside(&fixture)), ("b", None));
}

#[test]
fn the_split_key_splits_with_the_next_tab_and_unsplits_and_with_one_tab_opens_a_new_one() {
    let mut fixture = Fixture::with_tabs(&["a", "b"]);

    let _ = fixture.state.toggle_split();
    assert_eq!((fixture.active(), beside(&fixture)), ("b", Some("a")));
    let _ = fixture.state.toggle_split();
    assert_eq!(beside(&fixture), None);

    let mut single = Fixture::with_tabs(&["a"]);
    let _ = single.state.toggle_split();
    assert_eq!(single.state.tabs.len(), 2);
    assert_eq!(beside(&single), Some("a"));
    assert!(matches!(
        single.state.tabs[single.state.active].content,
        Content::Launcher(_)
    ));
}

#[test]
fn the_other_pane_key_gives_the_keyboard_across() {
    let mut fixture = Fixture::with_tabs(&["a", "b"]);
    let _ = fixture.state.toggle_split();

    fixture.state.focus_other_pane();

    assert_eq!((fixture.active(), beside(&fixture)), ("a", Some("b")));
}

fn split_pair() -> Fixture {
    let mut fixture = Fixture::with_tabs(&["a", "b"]);
    let _ = fixture.state.toggle_split();
    fixture
}

#[test]
fn a_split_starts_in_halves_side_by_side() {
    let fixture = split_pair();

    assert_eq!(fixture.state.share(), 0.5);
    assert!(!fixture.state.split_stacked);
}

#[test]
fn dragging_the_divider_sets_the_share_within_limits_and_only_while_held() {
    let mut fixture = split_pair();

    fixture.send(Message::DividerMoved(0.3));
    assert_eq!(fixture.state.share(), 0.5, "not held yet");

    fixture.send(Message::DividerPressed);
    fixture.send(Message::DividerMoved(0.3));
    assert_eq!(fixture.state.share(), 0.3);
    fixture.send(Message::DividerMoved(0.99));
    assert_eq!(fixture.state.share(), 0.8);
    fixture.send(Message::DividerMoved(-4.0));
    assert_eq!(fixture.state.share(), 0.2);
    fixture.send(Message::DividerMoved(f32::NAN));
    assert_eq!(fixture.state.share(), 0.2);

    fixture.state.release(false);
    fixture.send(Message::DividerMoved(0.6));
    assert_eq!(fixture.state.share(), 0.2, "let go");
}

#[test]
fn the_menu_stacks_the_panes_and_puts_them_side_by_side_again() {
    let mut fixture = split_pair();

    fixture.send(Message::MenuRequested(0));
    fixture.send(Message::MenuRotatePressed);
    assert!(fixture.state.split_stacked && fixture.state.menu.is_none());

    fixture.send(Message::MenuRequested(0));
    fixture.send(Message::MenuRotatePressed);
    assert!(!fixture.state.split_stacked);
}

#[test]
fn a_press_on_the_divider_does_nothing_without_a_split() {
    let mut fixture = Fixture::with_tabs(&["a"]);

    fixture.send(Message::DividerPressed);

    assert!(!fixture.state.resizing);
}
