//! Tests of `properties.rs`.

use super::*;
use remotething_core::{Credential, CredentialKind};

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

#[test]
fn port_forwards_are_typed_as_a_list_and_stored_parsed() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();
    draft.forwards = "8080:db.internal:5432, 9000:localhost:9000".to_string();

    let updated = draft.apply(&original, &web()).unwrap();

    let forwards = &updated.connections[1].settings.forwards;
    assert_eq!(forwards.len(), 2);
    assert_eq!(forwards[0].local_port, 8080);
    let reloaded = Draft::load(&updated, &web()).unwrap();
    assert_eq!(
        reloaded.forwards,
        "8080:db.internal:5432, 9000:localhost:9000"
    );
}

#[test]
fn a_remote_command_is_stored_trimmed_and_a_blank_one_clears_it() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();
    draft.remote_command = "  tmux new -A -s main ".to_string();

    let updated = draft.apply(&original, &web()).unwrap();

    let command = updated.connections[1].settings.remote_command.clone();
    assert_eq!(command.as_deref(), Some("tmux new -A -s main"));
    let mut cleared = Draft::load(&updated, &web()).unwrap();
    assert_eq!(cleared.remote_command, "tmux new -A -s main");
    cleared.remote_command = "   ".to_string();
    let updated = cleared.apply(&updated, &web()).unwrap();
    assert_eq!(updated.connections[1].settings.remote_command, None);
}

#[test]
fn a_broken_port_forward_names_the_text_and_changes_nothing() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();
    draft.forwards = "8080:db".to_string();

    let error = draft.apply(&original, &web()).unwrap_err();

    assert!(error.contains("8080:db"));
}

#[test]
fn a_picked_quality_is_stored_on_the_item() {
    let mut draft = Draft::load(&store(), &web()).unwrap();
    assert_eq!(draft.desktop_quality, None);

    update(
        &mut draft,
        Message::QualityPicked(QualityChoice(Some(Quality::LowBandwidth))),
    );
    let updated = draft.apply(&store(), &web()).unwrap();

    assert_eq!(
        updated.connections[1].settings.desktop.quality,
        Some(Quality::LowBandwidth)
    );
}

#[test]
fn choosing_inherit_clears_the_quality() {
    let mut draft = Draft::load(&store(), &web()).unwrap();
    draft.desktop_quality = Some(Quality::Lan);

    update(&mut draft, Message::QualityPicked(QualityChoice(None)));
    let updated = draft.apply(&store(), &web()).unwrap();

    assert_eq!(updated.connections[1].settings.desktop.quality, None);
}

fn typed(draft: &mut Draft, user: &str, password: &str) {
    update(draft, Message::NewUsernameChanged(user.to_string()));
    update(
        draft,
        Message::NewPasswordChanged(Hidden(password.to_string())),
    );
}

#[test]
fn a_password_typed_for_a_connection_without_a_credential_becomes_one_and_is_linked() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();
    typed(&mut draft, " ops ", "s3cret");

    let saved = draft.apply_with_password(&original, &web()).unwrap();

    let credential = saved.store.credentials.last().unwrap();
    assert_eq!(credential.name, "web-01");
    assert_eq!(credential.username.as_deref(), Some("ops"));
    assert_eq!(credential.kind, CredentialKind::Password);
    let connection = saved
        .store
        .connections
        .iter()
        .find(|connection| connection.id == "web-01")
        .unwrap();
    assert_eq!(
        connection.settings.credential.as_deref(),
        Some(credential.id.as_str())
    );
    let (id, secret) = saved.secret.unwrap();
    assert_eq!(id, credential.id);
    assert_eq!(secret.expose(), "s3cret");
}

#[test]
fn the_password_never_reaches_the_stored_description() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();
    typed(&mut draft, "ops", "s3cret");

    let saved = draft.apply_with_password(&original, &web()).unwrap();

    let text = saved.store.to_export().unwrap();
    assert!(!text.contains("s3cret"), "{text}");
}

#[test]
fn a_password_with_no_user_name_makes_a_credential_without_one() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();
    typed(&mut draft, "  ", "s3cret");

    let saved = draft.apply_with_password(&original, &web()).unwrap();

    assert_eq!(saved.store.credentials.last().unwrap().username, None);
}

#[test]
fn nothing_typed_makes_no_credential() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();
    typed(&mut draft, "ops", "");

    let saved = draft.apply_with_password(&original, &web()).unwrap();

    assert!(saved.secret.is_none());
    assert!(saved.store.credentials.is_empty());
}

#[test]
fn a_chosen_credential_is_kept_and_a_typed_password_is_ignored() {
    let mut original = store();
    original.credentials.push(Credential {
        id: "cred".to_string(),
        name: "Ops".to_string(),
        username: Some("ops".to_string()),
        domain: None,
        kind: CredentialKind::Ask,
        key_file: None,
    });
    let mut draft = Draft::load(&original, &web()).unwrap();
    draft.credential = Some("cred".to_string());
    typed(&mut draft, "someone", "s3cret");

    let saved = draft.apply_with_password(&original, &web()).unwrap();

    assert!(saved.secret.is_none());
    assert_eq!(saved.store.credentials.len(), 1);
}

#[test]
fn a_folder_never_takes_a_password() {
    let original = store();
    let folder = Selection::Folder("prod".to_string());
    let mut draft = Draft::load(&original, &folder).unwrap();
    typed(&mut draft, "ops", "s3cret");

    let saved = draft.apply_with_password(&original, &folder).unwrap();

    assert!(saved.secret.is_none());
}

#[test]
fn a_draft_prints_without_the_password() {
    let original = store();
    let mut draft = Draft::load(&original, &web()).unwrap();
    typed(&mut draft, "ops", "s3cret");

    assert!(!format!("{draft:?}").contains("s3cret"));
}

fn local_draft() -> Draft {
    let mut draft = Draft::load(&store(), &web()).unwrap();
    draft.protocol = Protocol::Local;
    draft.host = String::new();
    draft.directory = "~/src".to_string();
    draft.arguments = r#"-l --rcfile "my rc""#.to_string();
    draft.environment = "LANG=pl_PL.UTF-8 EDITOR=vim".to_string();
    draft
}

#[test]
fn a_local_shell_keeps_its_folder_arguments_and_variables_and_may_have_no_host() {
    let saved = local_draft().apply(&store(), &web()).unwrap();

    let connection = saved.connections.iter().find(|c| c.id == "web-01").unwrap();
    let local = &connection.settings.local;
    assert_eq!(connection.protocol, Protocol::Local);
    assert!(connection.host.is_empty());
    assert_eq!(local.directory.as_deref(), Some("~/src"));
    assert_eq!(local.arguments, ["-l", "--rcfile", "my rc"]);
    assert_eq!(local.environment["EDITOR"], "vim");

    // Opening the saved shell again shows the same text.
    let again = Draft::load(&saved, &web()).unwrap();
    // The quotes may change, the words do not.
    assert_eq!(
        local_shell::split_words(&again.arguments).unwrap(),
        ["-l", "--rcfile", "my rc"]
    );
    assert_eq!(again.directory, "~/src");
}

#[test]
fn a_shell_setting_that_does_not_parse_is_refused_with_a_reason() {
    let mut draft = local_draft();
    draft.environment = "NOVALUE".to_string();
    assert!(
        draft
            .apply(&store(), &web())
            .unwrap_err()
            .contains("NOVALUE")
    );

    let mut draft = local_draft();
    draft.arguments = "\"open".to_string();
    assert!(draft.apply(&store(), &web()).is_err());
}

#[test]
fn switching_away_from_a_local_shell_drops_its_settings() {
    let local = local_draft().apply(&store(), &web()).unwrap();
    let mut draft = Draft::load(&local, &web()).unwrap();
    draft.protocol = Protocol::Ssh;
    draft.host = "web-01.example.net".to_string();

    let saved = draft.apply(&local, &web()).unwrap();

    let connection = saved.connections.iter().find(|c| c.id == "web-01").unwrap();
    assert!(connection.settings.local.is_empty());
}

#[test]
fn choosing_a_local_shell_clears_the_address_a_new_connection_starts_with() {
    let mut draft = Draft::load(&store(), &web()).unwrap();
    draft.host = fl!("tree-new-connection-host");
    update(
        &mut draft,
        Message::ProtocolPicked(ProtocolChoice(Protocol::Local)),
    );
    assert_eq!(draft.host, "");

    // A name the user typed is theirs to keep.
    let mut draft = Draft::load(&store(), &web()).unwrap();
    update(
        &mut draft,
        Message::ProtocolPicked(ProtocolChoice(Protocol::Local)),
    );
    assert_eq!(draft.host, "web-01.example.net");
}

#[test]
fn a_terminal_theme_is_saved_with_the_connection_and_cleared_by_default() {
    let mut draft = Draft::load(&store(), &web()).unwrap();
    assert_eq!(draft.terminal_theme, None);

    update(
        &mut draft,
        Message::TerminalThemePicked(Choice {
            id: Some("Nord Dark".to_string()),
            label: "Nord Dark".to_string(),
        }),
    );
    let saved = draft.apply(&store(), &web()).unwrap();
    let connection = saved.connections.iter().find(|c| c.id == "web-01").unwrap();
    assert_eq!(
        connection.settings.terminal_theme.as_deref(),
        Some("Nord Dark")
    );

    update(
        &mut draft,
        Message::TerminalThemePicked(Choice {
            id: None,
            label: "Default".to_string(),
        }),
    );
    let saved = draft.apply(&store(), &web()).unwrap();
    let connection = saved.connections.iter().find(|c| c.id == "web-01").unwrap();
    assert_eq!(connection.settings.terminal_theme, None);
}
