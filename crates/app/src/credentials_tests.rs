//! Tests of `credentials.rs`.

use super::*;

fn store() -> Store {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("connections.toml");
    let text = r#"
        [[credential]]
        id = "cred-ops"
        name = "Ops team"
        username = "ops"
        kind = "private-key"
        key_file = "/keys/ops"
    "#;
    std::fs::write(&path, text).unwrap();
    Store::load(&path).unwrap()
}

fn selected_state() -> State {
    let mut state = State::default();
    state.select(&store(), Some("cred-ops".to_string()));
    state
}

#[test]
fn selecting_loads_the_form_with_an_empty_secret() {
    let state = selected_state();
    let draft = state.draft().unwrap();

    assert_eq!(draft.name, "Ops team");
    assert_eq!(draft.username, "ops");
    assert_eq!(draft.key_file, "/keys/ops");
    assert_eq!(draft.secret, "");
}

#[test]
fn applying_writes_the_description_but_never_the_secret() {
    let original = store();
    let mut state = selected_state();
    update(
        &mut state,
        &original,
        Message::NameChanged("Operations".to_string()),
    );
    update(
        &mut state,
        &original,
        Message::DomainChanged("CORP".to_string()),
    );
    update(
        &mut state,
        &original,
        Message::SecretChanged(Hidden("hunter2".to_string())),
    );

    let updated = state.draft().unwrap().apply(&original, "cred-ops").unwrap();

    assert_eq!(updated.credentials[0].name, "Operations");
    assert_eq!(updated.credentials[0].domain.as_deref(), Some("CORP"));
    let file_text = toml_text(&updated);
    assert!(!file_text.contains("hunter2"));
}

fn toml_text(store: &Store) -> String {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("connections.toml");
    store.save(&path).unwrap();
    std::fs::read_to_string(&path).unwrap()
}

#[test]
fn switching_away_from_private_key_drops_the_key_file() {
    let original = store();
    let mut state = selected_state();
    let password = KindChoice(CredentialKind::Password);
    update(&mut state, &original, Message::KindPicked(password));

    let updated = state.draft().unwrap().apply(&original, "cred-ops").unwrap();

    assert_eq!(updated.credentials[0].key_file, None);
}

#[test]
fn an_empty_name_is_refused() {
    let original = store();
    let mut state = selected_state();
    update(&mut state, &original, Message::NameChanged(" ".to_string()));

    assert!(state.draft().unwrap().apply(&original, "cred-ops").is_err());
}

#[test]
fn a_typed_secret_is_stored_and_the_field_is_emptied() {
    let mut state = selected_state();
    update(
        &mut state,
        &store(),
        Message::SecretChanged(Hidden("hunter2".to_string())),
    );

    let action = state.take_secret_action();

    assert!(matches!(action, SecretAction::Set(secret) if secret.expose() == "hunter2"));
    assert_eq!(state.draft().unwrap().secret, "");
}

#[test]
fn an_empty_secret_field_keeps_the_stored_secret() {
    let mut state = selected_state();

    assert!(matches!(state.take_secret_action(), SecretAction::Keep));
}

#[test]
fn ask_every_time_deletes_the_stored_secret() {
    let mut state = selected_state();
    let ask = KindChoice(CredentialKind::Ask);
    update(&mut state, &store(), Message::KindPicked(ask));
    update(
        &mut state,
        &store(),
        Message::SecretChanged(Hidden("typed anyway".to_string())),
    );

    assert!(matches!(state.take_secret_action(), SecretAction::Delete));
}

#[test]
fn a_typed_secret_does_not_appear_when_the_message_is_printed() {
    let message = Message::SecretChanged(Hidden("hunter2".to_string()));

    assert!(!format!("{message:?}").contains("hunter2"));
}

#[test]
fn buttons_are_reported_to_the_window() {
    let mut state = selected_state();
    let store = store();

    assert_eq!(
        update(&mut state, &store, Message::NewPressed),
        Some(Event::New)
    );
    assert_eq!(
        update(&mut state, &store, Message::SavePressed),
        Some(Event::Save)
    );
    assert_eq!(
        update(&mut state, &store, Message::DeletePressed),
        Some(Event::Delete)
    );
    assert_eq!(
        update(&mut state, &store, Message::ClosePressed),
        Some(Event::Close)
    );
}
