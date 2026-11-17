//! Tests of `palette.rs`.

use super::*;

fn store() -> Store {
    let text = r#"
        [[connection]]
        id = "web-01"
        name = "web-01"
        protocol = "ssh"
        host = "web-01.example.net"

        [[connection]]
        id = "db-01"
        name = "Database"
        protocol = "rdp"
        host = "db-01.example.net"
    "#;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("connections.toml");
    std::fs::write(&path, text).unwrap();
    Store::load(&path).unwrap()
}

fn items(entries: &[Entry]) -> Vec<Item> {
    entries.iter().map(|entry| entry.item.clone()).collect()
}

fn connection(id: &str) -> Item {
    Item::Connection(id.to_string())
}

#[test]
fn without_a_query_the_recent_connections_come_before_the_actions() {
    let recent = vec!["db-01".to_string()];

    let found = entries(&store(), &recent, "");

    assert_eq!(
        items(&found),
        [
            connection("db-01"),
            Item::NewTab,
            Item::Credentials,
            Item::Settings
        ]
    );
}

#[test]
fn a_query_finds_connections_by_name_or_host_and_actions_by_label() {
    let by_host = entries(&store(), &[], "example");
    assert_eq!(items(&by_host), [connection("web-01"), connection("db-01")]);

    let action = entries(&store(), &[], "setting");
    assert_eq!(items(&action), [Item::Settings]);

    assert!(entries(&store(), &[], "zzz").is_empty());
}

#[test]
fn enter_chooses_the_highlighted_line() {
    let found = entries(&store(), &[], "example");
    let mut state = State::default();
    state.step(true, found.len());

    let event = update(&mut state, Message::Submitted, &found);

    assert_eq!(event, Some(Event::Chosen(connection("db-01"))));
}

#[test]
fn the_highlight_wraps_round_both_ends() {
    let mut state = State::default();

    state.step(false, 3);
    assert_eq!(state.selected, 2);
    state.step(true, 3);
    assert_eq!(state.selected, 0);
    // Nothing to move over.
    state.step(true, 0);
    assert_eq!(state.selected, 0);
}

#[test]
fn typing_puts_the_highlight_back_on_the_first_line() {
    let mut state = State::default();
    state.step(true, 3);

    update(&mut state, Message::Typed("w".to_string()), &[]);

    assert_eq!(state.selected, 0);
    assert_eq!(state.query(), "w");
}

#[test]
fn a_click_chooses_that_line_and_the_backdrop_closes() {
    let found = entries(&store(), &[], "web");
    let mut state = State::default();

    assert_eq!(
        update(&mut state, Message::Picked(0), &found),
        Some(Event::Chosen(connection("web-01")))
    );
    assert_eq!(update(&mut state, Message::Picked(9), &found), None);
    assert_eq!(
        update(&mut state, Message::Dismissed, &found),
        Some(Event::Close)
    );
}
