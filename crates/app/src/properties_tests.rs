//! Tests of `properties.rs`.

use super::*;

fn store() -> Store {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("connections.toml");
    let text = r#"
        [[folder]]
        id = "prod"
        name = "Production"
        port = 2222

        [[connection]]
        id = "bastion"
        name = "bastion"
        protocol = "ssh"
        host = "bastion.example.net"

        [[connection]]
        id = "web-01"
        name = "web-01"
        parent = "prod"
        protocol = "ssh"
        host = "web-01.example.net"
    "#;
    std::fs::write(&path, text).unwrap();
    Store::load(&path).unwrap()
}

fn web() -> Selection {
    Selection::Connection("web-01".to_string())
}

#[test]
fn a_draft_shows_the_items_own_values_not_inherited_ones() {
    let draft = Draft::load(&store(), &web()).unwrap();

    assert_eq!(draft.name, "web-01");
    assert_eq!(draft.host, "web-01.example.net");
    // The folder sets 2222, but the connection itself sets nothing.
    assert_eq!(draft.port, "");
    assert_eq!(draft.parent.as_deref(), Some("prod"));
}

#[test]
fn a_missing_item_has_no_draft() {
    let gone = Selection::Connection("gone".to_string());

    assert_eq!(Draft::load(&store(), &gone), None);
}

#[test]
fn applying_writes_the_fields_into_a_copy() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();
    draft.name = "web".to_string();
    draft.port = "8022".to_string();
    draft.jump = Some("bastion".to_string());
    draft.protocol = Protocol::Rdp;

    let updated = draft.apply(&original, &web()).unwrap();

    let connection = &updated.connections[1];
    assert_eq!(connection.name, "web");
    assert_eq!(connection.settings.port, Some(8022));
    assert_eq!(connection.settings.jump.as_deref(), Some("bastion"));
    assert_eq!(connection.protocol, Protocol::Rdp);
    assert_eq!(original.connections[1].name, "web-01");
}

#[test]
fn picking_an_icon_saves_its_name_and_the_default_clears_it() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();
    assert_eq!(draft.icon, None);

    update(&mut draft, Message::IconPicked(Some("database")));
    let updated = draft.apply(&original, &web()).unwrap();
    assert_eq!(updated.connections[1].icon.as_deref(), Some("database"));

    let mut draft = Draft::load(&updated, &web()).unwrap();
    assert_eq!(draft.icon.as_deref(), Some("database"));
    update(&mut draft, Message::IconPicked(None));
    let cleared = draft.apply(&updated, &web()).unwrap();
    assert_eq!(cleared.connections[1].icon, None);
}

#[test]
fn clearing_the_port_goes_back_to_inheriting() {
    let folder = Selection::Folder("prod".to_string());
    let original = store();
    let mut draft = Draft::load(&original, &folder).unwrap();
    assert_eq!(draft.port, "2222");
    draft.port = " ".to_string();

    let updated = draft.apply(&original, &folder).unwrap();

    assert_eq!(updated.folders[0].settings.port, None);
}

#[test]
fn a_bad_port_is_refused() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();

    for bad in ["ssh", "0", "70000", "-1"] {
        draft.port = bad.to_string();
        assert!(draft.apply(&original, &web()).is_err(), "port {bad}");
    }
}

#[test]
fn an_empty_name_or_host_is_refused() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();

    draft.name = "  ".to_string();
    assert!(draft.apply(&original, &web()).is_err());

    draft.name = "web".to_string();
    draft.host = String::new();
    assert!(draft.apply(&original, &web()).is_err());
}

#[test]
fn a_change_that_breaks_the_tree_is_refused() {
    let folder = Selection::Folder("prod".to_string());
    let original = store();
    let mut draft = Draft::load(&original, &folder).unwrap();
    draft.parent = Some("prod".to_string());

    assert!(draft.apply(&original, &folder).is_err());
}

#[test]
fn a_folder_is_not_offered_as_its_own_parent() {
    let folder = Selection::Folder("prod".to_string());

    let choices = parent_choices(&store(), &folder);

    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].id, None);
}

#[test]
fn a_connection_is_not_offered_as_its_own_jump_host() {
    let ids: Vec<Option<String>> = jump_choices(&store(), &web())
        .into_iter()
        .map(|choice| choice.id)
        .collect();

    assert_eq!(ids, [None, Some("bastion".to_string())]);
}

#[test]
fn save_and_delete_are_reported_to_the_window() {
    let mut draft = Draft::load(&store(), &web()).unwrap();

    assert_eq!(update(&mut draft, Message::SavePressed), Some(Event::Save));
    assert_eq!(
        update(&mut draft, Message::DeletePressed),
        Some(Event::Delete)
    );
    assert_eq!(
        update(&mut draft, Message::NameChanged("x".to_string())),
        None
    );
    assert_eq!(draft.name, "x");
}
