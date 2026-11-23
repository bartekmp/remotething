//! Tests of `items.rs`.

use std::sync::Arc;

use iced::Point;
use remotething_core::Store;
use remotething_secrets::MemoryStore;

use super::*;
use crate::app::update;
use crate::{menu, tree};

const TREE: &str = r#"
    [[folder]]
    id = "prod"
    name = "Production"

    [[folder]]
    id = "lab"
    name = "Lab"

    [[connection]]
    id = "web-01"
    name = "web-01"
    parent = "prod"
    protocol = "ssh"
    host = "web-01.example.net"
"#;

fn state(directory: &tempfile::TempDir) -> State {
    let path = directory.path().join("connections.toml");
    std::fs::write(&path, TREE).unwrap();
    let store = Store::load(&path).unwrap();
    State::new(path, Ok(store), Arc::new(MemoryStore::new()), None)
}

fn send(state: &mut State, message: Message) {
    let _ = update(state, message);
}

fn right_click(state: &mut State, target: Option<Selection>) {
    send(state, Message::PointerMoved(Point::new(40.0, 90.0)));
    send(state, Message::Tree(tree::Message::MenuRequested(target)));
}

fn escape() -> Message {
    Message::KeyPressed {
        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
        physical: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Escape),
        modifiers: iced::keyboard::Modifiers::empty(),
        text: None,
        captured: false,
    }
}

fn folder(id: &str) -> Selection {
    Selection::Folder(id.to_string())
}

fn connection(id: &str) -> Selection {
    Selection::Connection(id.to_string())
}

fn on_disk(state: &State) -> Store {
    Store::load(&state.path).unwrap()
}

#[test]
fn a_right_click_opens_the_menu_at_the_pointer_and_selects_the_row() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    right_click(&mut state, Some(connection("web-01")));

    let expected = menu::State::new(Some(connection("web-01")), Point::new(40.0, 90.0));
    assert_eq!(state.menu, Some(expected));
    assert_eq!(state.tree.selected(), Some(&connection("web-01")));
}

#[test]
fn escape_and_a_click_outside_close_the_menu() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    right_click(&mut state, None);
    send(&mut state, escape());
    assert_eq!(state.menu, None);

    right_click(&mut state, None);
    send(&mut state, Message::Menu(menu::Message::Dismissed));
    assert_eq!(state.menu, None);
}

#[test]
fn new_connection_on_a_folder_goes_inside_it_whatever_is_selected() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    state.select(Some(folder("prod")));

    right_click(&mut state, Some(folder("lab")));
    send(&mut state, Message::Menu(menu::Message::NewConnection));

    let added = on_disk(&state).connections.pop().unwrap();
    assert_eq!(added.parent.as_deref(), Some("lab"));
    assert_eq!(state.menu, None);
}

#[test]
fn new_folder_on_the_empty_space_goes_to_the_top_level() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    state.select(Some(folder("prod")));

    right_click(&mut state, None);
    send(&mut state, Message::Menu(menu::Message::NewFolder));

    assert_eq!(on_disk(&state).folders.pop().unwrap().parent, None);
}

#[test]
fn connect_in_the_menu_opens_a_tab() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    right_click(&mut state, Some(connection("web-01")));
    send(&mut state, Message::Menu(menu::Message::Connect));

    assert_eq!(state.tabs.open_ids(), ["web-01"]);
}

#[test]
fn rename_in_the_menu_then_enter_saves_the_new_name() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    right_click(&mut state, Some(folder("prod")));
    send(&mut state, Message::Menu(menu::Message::Rename));
    let typed = tree::Message::RenameChanged("Live".to_string());
    send(&mut state, Message::Tree(typed));
    send(&mut state, Message::Tree(tree::Message::RenameSubmitted));

    assert_eq!(on_disk(&state).folders[0].name, "Live");
    // The properties panel shows the new name too.
    assert_eq!(state.draft.as_ref().unwrap().name, "Live");
}

#[test]
fn escape_during_a_rename_keeps_the_old_name() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    right_click(&mut state, Some(folder("prod")));
    send(&mut state, Message::Menu(menu::Message::Rename));
    let typed = tree::Message::RenameChanged("Live".to_string());
    send(&mut state, Message::Tree(typed));
    send(&mut state, escape());
    send(&mut state, Message::Tree(tree::Message::RenameSubmitted));

    assert_eq!(on_disk(&state).folders[0].name, "Production");
}

#[test]
fn change_icon_keeps_the_menu_open_until_an_icon_is_picked() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    right_click(&mut state, Some(connection("web-01")));
    send(&mut state, Message::Menu(menu::Message::ChangeIcon));
    assert!(state.menu.is_some());

    let picked = menu::Message::IconPicked(Some("database"));
    send(&mut state, Message::Menu(picked));

    assert_eq!(state.menu, None);
    let saved = on_disk(&state);
    assert_eq!(saved.connections[0].icon.as_deref(), Some("database"));
}

#[test]
fn remove_in_the_menu_deletes_the_item_and_asks_about_a_full_folder() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    right_click(&mut state, Some(folder("prod")));
    send(&mut state, Message::Menu(menu::Message::Remove));
    assert!(state.removal.is_some());
    assert_eq!(on_disk(&state).folders.len(), 2);
    state.removal = None;

    right_click(&mut state, Some(folder("lab")));
    send(&mut state, Message::Menu(menu::Message::Remove));
    assert_eq!(on_disk(&state).folders.len(), 1);
}

#[test]
fn dragging_a_connection_onto_a_folder_moves_it_on_disk() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    let pressed = tree::Message::ConnectionSelected("web-01".to_string());

    send(&mut state, Message::Tree(pressed));
    let entered = tree::Message::RowEntered(folder("lab"));
    send(&mut state, Message::Tree(entered));
    send(&mut state, Message::PointerReleased);

    let saved = on_disk(&state);
    assert_eq!(saved.connections[0].parent.as_deref(), Some("lab"));
    assert_eq!(state.draft.as_ref().unwrap().parent.as_deref(), Some("lab"));
}

#[test]
fn a_refused_drop_changes_nothing_and_says_why() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    state.move_item(&folder("lab"), &DropTarget::Row(folder("prod")));

    state.move_item(&folder("prod"), &DropTarget::Row(folder("lab")));

    assert!(state.error.is_some());
    assert_eq!(on_disk(&state).folders[0].parent, None);
}

#[test]
fn the_properties_panel_stays_closed_when_a_row_is_only_selected() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    let pressed = tree::Message::ConnectionSelected("web-01".to_string());
    send(&mut state, Message::Tree(pressed));

    assert!(!state.properties_open);
}

#[test]
fn edit_in_the_menu_opens_the_panel_and_saving_closes_it() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    right_click(&mut state, Some(connection("web-01")));
    send(&mut state, Message::Menu(menu::Message::Edit));
    assert!(state.properties_open);

    let save = crate::properties::Message::SavePressed;
    send(&mut state, Message::Properties(save));
    assert!(!state.properties_open);
}

#[test]
fn a_refused_save_keeps_the_panel_open() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    right_click(&mut state, Some(connection("web-01")));
    send(&mut state, Message::Menu(menu::Message::Edit));

    let cleared = crate::properties::Message::NameChanged(String::new());
    send(&mut state, Message::Properties(cleared));
    let save = crate::properties::Message::SavePressed;
    send(&mut state, Message::Properties(save));

    assert!(state.properties_open);
    assert!(state.error.is_some());
}

#[test]
fn the_close_button_and_escape_close_the_panel() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    state.add_connection(None);
    assert!(state.properties_open);
    send(&mut state, Message::PropertiesClosed);
    assert!(!state.properties_open);

    state.add_folder(None);
    send(&mut state, escape());
    assert!(!state.properties_open);
}

#[test]
fn an_export_imported_into_the_same_tree_doubles_it_without_touching_what_was_there() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    let file = directory.path().join("export.json");

    state.export_to(&file);
    assert!(std::fs::read_to_string(&file).unwrap().contains("web-01"));

    state.preview_import(&file);
    assert_eq!(state.centre(), Centre::Import);
    send(
        &mut state,
        Message::Preview(import_preview::Message::ImportPressed),
    );

    assert!(state.error.is_none());
    assert!(state.notice.is_some());
    assert_eq!(state.centre(), Centre::Settings);
    let saved = on_disk(&state);
    assert_eq!(saved.folders.len(), 4);
    assert_eq!(saved.connections.len(), 2);
    assert_eq!(saved.connections[0].id, "web-01");
    assert_eq!(saved.connections[1].id, "web-01-2");
}

#[test]
fn a_file_that_is_not_an_export_changes_nothing_and_says_why() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    let before = on_disk(&state);
    let file = directory.path().join("other.json");
    std::fs::write(&file, "{}").unwrap();

    state.preview_import(&file);

    assert_eq!(on_disk(&state), before);
    assert!(state.import.is_none());
    assert!(state.error.is_some());
}

#[test]
fn saving_a_typed_connection_adds_it_with_its_user_and_opens_the_form_filled_in() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    let target = remotething_core::parse_quick("ssh ops@10.0.0.5:2222").unwrap();

    state.save_typed_connection(0, &target);

    let saved = on_disk(&state);
    let connection = saved.connections.last().unwrap();
    assert_eq!(connection.host, "10.0.0.5");
    assert_eq!(connection.settings.port, Some(2222));
    let credential = saved.credentials.last().unwrap();
    assert_eq!(credential.username.as_deref(), Some("ops"));
    assert_eq!(credential.kind, CredentialKind::Ask);
    assert_eq!(
        connection.settings.credential.as_ref(),
        Some(&credential.id)
    );

    assert!(state.properties_open);
    let draft = state.draft.as_ref().unwrap();
    assert_eq!(draft.host, "10.0.0.5");
    assert_eq!(draft.port, "2222");
    assert_eq!(state.recent.ids()[0], connection.id);
}

#[test]
fn saving_a_typed_connection_without_a_user_adds_no_credential() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    let target = remotething_core::parse_quick("rdp://desk").unwrap();

    state.save_typed_connection(0, &target);

    let saved = on_disk(&state);
    assert!(saved.credentials.is_empty());
    assert_eq!(saved.connections.last().unwrap().settings.port, None);
}

#[test]
fn a_plain_click_release_moves_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    let before = on_disk(&state);

    let pressed = tree::Message::ConnectionSelected("web-01".to_string());
    send(&mut state, Message::Tree(pressed));
    send(&mut state, Message::PointerReleased);

    assert_eq!(on_disk(&state), before);
}

#[test]
fn move_down_in_the_menu_reorders_folders_on_disk() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    right_click(&mut state, Some(folder("prod")));
    send(&mut state, Message::Menu(menu::Message::MoveDown));

    let ids: Vec<String> = on_disk(&state).folders.into_iter().map(|f| f.id).collect();
    assert_eq!(ids, ["lab", "prod"]);
}

#[test]
fn move_up_on_the_first_folder_changes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);

    right_click(&mut state, Some(folder("prod")));
    send(&mut state, Message::Menu(menu::Message::MoveUp));

    let ids: Vec<String> = on_disk(&state).folders.into_iter().map(|f| f.id).collect();
    assert_eq!(ids, ["prod", "lab"]);
}

#[test]
fn importing_with_nothing_ticked_changes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = state(&directory);
    let file = directory.path().join("export.json");
    state.export_to(&file);
    state.preview_import(&file);

    let untick = import_preview::Message::Toggled("web-01".to_string(), false);
    send(&mut state, Message::Preview(untick));
    send(
        &mut state,
        Message::Preview(import_preview::Message::ImportPressed),
    );

    // Nothing was ticked, so nothing happened and the screen stays.
    assert_eq!(on_disk(&state).connections.len(), 1);
    assert_eq!(on_disk(&state).folders.len(), 2);
}
