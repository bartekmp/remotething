//! The master-password prompt, shown when there is no system keyring.
//! It creates or unlocks the encrypted passwords file.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::widget::{button, column, row, text};
use iced::{Element, Length};
use remotething_secrets::{Error, FileStore, Secret, SecretStore};

use crate::credentials::Hidden;
use crate::i18n::fl;
use crate::theme;

const FORM_WIDTH: f32 = 420.0;

/// The prompt as typed. It holds the master password while it is entered, so it has no `Debug`.
pub struct State {
    path: PathBuf,
    /// True when the file exists and must be unlocked; false when it will be created.
    exists: bool,
    /// Why the keyring is not used, shown for context.
    reason: String,
    password: String,
    confirmation: String,
    error: Option<String>,
    working: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    PasswordChanged(Hidden),
    ConfirmationChanged(Hidden),
    SubmitPressed,
    SkipPressed,
}

/// What the prompt asks the window to do.
pub enum Event {
    Open(Request),
    Skip,
}

/// Everything needed to create or unlock the file, off the UI thread.
pub struct Request {
    path: PathBuf,
    master_password: Secret,
    create: bool,
}

/// The passwords file lives in the same directory as the connections file.
pub fn path_beside(connections_file: &Path) -> PathBuf {
    connections_file.with_file_name("secrets.toml")
}

impl State {
    pub fn new(path: PathBuf, reason: String) -> Self {
        Self {
            exists: path.exists(),
            path,
            reason,
            password: String::new(),
            confirmation: String::new(),
            error: None,
            working: false,
        }
    }

    /// Shows why creating or unlocking failed and lets the user try again.
    pub fn fail(&mut self, reason: String) {
        self.error = Some(reason);
        self.working = false;
    }

    /// Checks the typed passwords; on success empties the fields and hands them over.
    fn submit(&mut self) -> Option<Event> {
        if self.working {
            return None;
        }
        if self.password.is_empty() {
            self.error = Some(fl!("vault-error-empty"));
            return None;
        }
        if !self.exists && self.password != self.confirmation {
            self.error = Some(fl!("vault-error-mismatch"));
            return None;
        }

        self.confirmation.clear();
        self.error = None;
        self.working = true;
        Some(Event::Open(Request {
            path: self.path.clone(),
            master_password: Secret::new(std::mem::take(&mut self.password)),
            create: !self.exists,
        }))
    }
}

pub fn update(state: &mut State, message: Message) -> Option<Event> {
    match message {
        Message::PasswordChanged(typed) => state.password = typed.0,
        Message::ConfirmationChanged(typed) => state.confirmation = typed.0,
        Message::SubmitPressed => return state.submit(),
        Message::SkipPressed => return Some(Event::Skip),
    }
    None
}

/// Creates or unlocks the file. Key derivation is deliberately slow, so run this off the UI thread.
pub fn open(request: Request) -> Result<Arc<dyn SecretStore>, String> {
    if let Some(directory) = request.path.parent() {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    }
    let opened = if request.create {
        FileStore::create(&request.path, &request.master_password)
    } else {
        FileStore::open(&request.path, &request.master_password)
    };
    match opened {
        Ok(store) => Ok(Arc::new(store)),
        Err(Error::WrongPassword) => Err(fl!("vault-error-wrong")),
        Err(error) => Err(error.to_string()),
    }
}

pub fn view(state: &State) -> Element<'_, Message> {
    let (title, explanation, action) = if state.exists {
        (
            fl!("vault-title-open"),
            fl!("vault-explain-open"),
            fl!("vault-open"),
        )
    } else {
        (
            fl!("vault-title-create"),
            fl!("vault-explain-create"),
            fl!("vault-create"),
        )
    };

    let mut form = column![
        text(title).size(theme::TEXT_DISPLAY),
        text(explanation),
        text(fl!("vault-reason", reason = state.reason.as_str()))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        secret_field(
            fl!("vault-password"),
            &state.password,
            Message::PasswordChanged
        ),
    ]
    .spacing(12)
    .width(Length::Fixed(FORM_WIDTH));

    if !state.exists {
        form = form.push(secret_field(
            fl!("vault-confirm"),
            &state.confirmation,
            Message::ConfirmationChanged,
        ));
    }
    if let Some(error) = &state.error {
        form = form.push(text(error.as_str()).style(text::danger));
    }

    let submit = if state.working {
        button(text(fl!("vault-working"))).style(theme::primary)
    } else {
        button(text(action))
            .style(theme::primary)
            .on_press(Message::SubmitPressed)
    };
    let skip = button(text(fl!("vault-skip")))
        .style(theme::pill)
        .on_press(Message::SkipPressed);

    form.push(row![submit, skip].spacing(8)).into()
}

fn secret_field<'a>(
    label: String,
    value: &str,
    on_input: fn(Hidden) -> Message,
) -> Element<'a, Message> {
    let input = theme::input("", value)
        .secure(true)
        .on_input(move |typed| on_input(Hidden(typed)))
        .on_submit(Message::SubmitPressed);
    column![
        text(label)
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        input
    ]
    .spacing(2)
    .into()
}

#[cfg(test)]
mod tests {
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
}
