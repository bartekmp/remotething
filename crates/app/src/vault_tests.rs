//! Tests of `vault.rs`.

use super::*;

fn typed(text: &str) -> Hidden {
    Hidden(text.to_string())
}

fn new_state(directory: &tempfile::TempDir) -> State {
    let path = directory.path().join("config").join("secrets.toml");
    State::new(path, "no keyring".to_string())
}

/// Types the passwords and presses the button; returns the request if one was made.
fn submit(state: &mut State, password: &str, confirmation: &str) -> Option<Request> {
    update(state, Message::PasswordChanged(typed(password)));
    update(state, Message::ConfirmationChanged(typed(confirmation)));
    match update(state, Message::SubmitPressed) {
        Some(Event::Open(request)) => Some(request),
        _ => None,
    }
}

#[test]
fn creating_needs_the_same_password_twice() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = new_state(&directory);

    assert!(submit(&mut state, "master", "mastre").is_none());
    assert!(state.error.is_some());
    assert!(!state.working);
}

#[test]
fn an_empty_password_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = new_state(&directory);

    assert!(submit(&mut state, "", "").is_none());
    assert!(state.error.is_some());
}

#[test]
fn a_good_submit_empties_the_fields_and_cannot_be_pressed_twice() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = new_state(&directory);

    let request = submit(&mut state, "master", "master").unwrap();

    assert!(request.create);
    assert_eq!(request.master_password.expose(), "master");
    assert_eq!(state.password, "");
    assert_eq!(state.confirmation, "");
    assert!(state.working);
    assert!(update(&mut state, Message::SubmitPressed).is_none());
}

#[test]
fn create_then_unlock_gives_back_the_stored_secret() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = new_state(&directory);
    let request = submit(&mut state, "master", "master").unwrap();
    let path = request.path.clone();

    let store = open(request).unwrap();
    store
        .set("cred-ops", &Secret::new("hunter2".to_string()))
        .unwrap();
    drop(store);

    // A new prompt for the same path now unlocks instead of creating.
    let mut state = State::new(path, "no keyring".to_string());
    assert!(state.exists);
    let request = submit(&mut state, "master", "").unwrap();
    assert!(!request.create);
    let reopened = open(request).unwrap();

    let secret = reopened.get("cred-ops").unwrap().unwrap();
    assert_eq!(secret.expose(), "hunter2");
}

#[test]
fn a_wrong_master_password_gives_a_clear_message_and_another_try() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = new_state(&directory);
    let request = submit(&mut state, "master", "master").unwrap();
    let path = request.path.clone();
    open(request).unwrap();

    let mut state = State::new(path, "no keyring".to_string());
    let request = submit(&mut state, "not the master", "").unwrap();
    let Err(reason) = open(request) else {
        panic!("a wrong password must not unlock the file");
    };
    state.fail(reason);

    assert_eq!(
        state.error.as_deref(),
        Some(fl!("vault-error-wrong").as_str())
    );
    assert!(!state.working);
}

#[test]
fn typed_passwords_do_not_appear_when_messages_are_printed() {
    let message = Message::PasswordChanged(typed("master"));

    assert!(!format!("{message:?}").contains("master"));
}

#[test]
fn the_passwords_file_sits_beside_the_connections_file() {
    let path = path_beside(Path::new("/config/remotething/connections.toml"));

    assert_eq!(path, PathBuf::from("/config/remotething/secrets.toml"));
}
