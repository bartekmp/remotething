//! The credentials screen: who to log in as, and the secret that goes to the keyring.

use std::fmt;
use std::path::PathBuf;

use iced::widget::{button, column, row, scrollable, text};
use iced::{Element, Length};
use remotething_core::{CredentialKind, Store};
use remotething_secrets::Secret;

use crate::i18n::fl;
use crate::theme;

const LIST_WIDTH: f32 = 220.0;

const KINDS: [KindChoice; 3] = [
    KindChoice(CredentialKind::Password),
    KindChoice(CredentialKind::PrivateKey),
    KindChoice(CredentialKind::Ask),
];

/// Typed text that must not show up when a message is printed.
#[derive(Clone)]
pub struct Hidden(pub String);

impl fmt::Debug for Hidden {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Hidden(***)")
    }
}

/// The form as typed. It holds the secret while it is being entered, so it has no `Debug`.
#[derive(Clone)]
pub struct Draft {
    pub name: String,
    pub username: String,
    pub domain: String,
    pub kind: CredentialKind,
    pub key_file: String,
    /// Empty means "keep whatever is stored".
    pub secret: String,
}

#[derive(Default)]
pub struct State {
    selected: Option<String>,
    draft: Option<Draft>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Selected(String),
    NewPressed,
    NameChanged(String),
    UsernameChanged(String),
    DomainChanged(String),
    KindPicked(KindChoice),
    KeyFileChanged(String),
    SecretChanged(Hidden),
    SavePressed,
    DeletePressed,
    ClosePressed,
}

/// What the screen asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    New,
    Save,
    Delete,
    Close,
}

/// What to do in the secret store after a credential was saved.
pub enum SecretAction {
    Keep,
    Set(Secret),
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindChoice(CredentialKind);

impl fmt::Display for KindChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self.0 {
            CredentialKind::Password => fl!("kind-password"),
            CredentialKind::PrivateKey => fl!("kind-private-key"),
            CredentialKind::Ask => fl!("kind-ask"),
        };
        formatter.write_str(&label)
    }
}

impl State {
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn draft(&self) -> Option<&Draft> {
        self.draft.as_ref()
    }

    /// Shows the given credential in the form, or clears the form.
    pub fn select(&mut self, store: &Store, credential_id: Option<String>) {
        self.draft = credential_id
            .as_deref()
            .and_then(|id| Draft::load(store, id));
        self.selected = credential_id;
    }

    /// Decides what happens to the stored secret, and empties the typed one.
    pub fn take_secret_action(&mut self) -> SecretAction {
        let Some(draft) = &mut self.draft else {
            return SecretAction::Keep;
        };
        let typed = std::mem::take(&mut draft.secret);

        if draft.kind == CredentialKind::Ask {
            // "Ask every time" must not leave an old password behind.
            return SecretAction::Delete;
        }
        if typed.is_empty() {
            return SecretAction::Keep;
        }
        SecretAction::Set(Secret::new(typed))
    }
}

impl Draft {
    fn load(store: &Store, credential_id: &str) -> Option<Draft> {
        let credential = store
            .credentials
            .iter()
            .find(|credential| credential.id == credential_id)?;
        let key_file = credential
            .key_file
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();

        Some(Draft {
            name: credential.name.clone(),
            username: credential.username.clone().unwrap_or_default(),
            domain: credential.domain.clone().unwrap_or_default(),
            kind: credential.kind,
            key_file,
            secret: String::new(),
        })
    }

    /// Returns a copy of the store with this draft written into the credential.
    /// The secret is not part of the store and is handled separately.
    pub fn apply(&self, store: &Store, credential_id: &str) -> Result<Store, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(fl!("credentials-error-name"));
        }

        let mut updated = store.clone();
        let credential = updated
            .credentials
            .iter_mut()
            .find(|credential| credential.id == credential_id);
        if let Some(credential) = credential {
            credential.name = name.to_string();
            credential.username = non_empty(&self.username);
            credential.domain = non_empty(&self.domain);
            credential.kind = self.kind;
            credential.key_file = match self.kind {
                CredentialKind::PrivateKey => non_empty(&self.key_file).map(PathBuf::from),
                _ => None,
            };
        }

        updated.validate().map_err(|error| error.to_string())?;
        Ok(updated)
    }
}

fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.to_string())
}

pub fn update(state: &mut State, store: &Store, message: Message) -> Option<Event> {
    match message {
        Message::Selected(id) => {
            state.select(store, Some(id));
            return None;
        }
        Message::NewPressed => return Some(Event::New),
        Message::SavePressed => return Some(Event::Save),
        Message::DeletePressed => return Some(Event::Delete),
        Message::ClosePressed => return Some(Event::Close),
        _ => {}
    }

    let draft = state.draft.as_mut()?;
    match message {
        Message::NameChanged(name) => draft.name = name,
        Message::UsernameChanged(username) => draft.username = username,
        Message::DomainChanged(domain) => draft.domain = domain,
        Message::KindPicked(choice) => draft.kind = choice.0,
        Message::KeyFileChanged(path) => draft.key_file = path,
        Message::SecretChanged(typed) => draft.secret = typed.0,
        _ => {}
    }
    None
}

pub fn view<'a>(state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let header = row![
        text(fl!("credentials-title"))
            .size(theme::TEXT_DISPLAY)
            .width(Length::Fill),
        button(text(fl!("credentials-close")))
            .style(theme::pill)
            .on_press(Message::ClosePressed),
    ];

    let entries = store.credentials.iter().map(|credential| {
        let is_selected = state.selected() == Some(credential.id.as_str());
        button(text(credential.name.as_str()))
            .style(theme::entry(is_selected))
            .width(Length::Fill)
            .on_press(Message::Selected(credential.id.clone()))
            .into()
    });
    let list = column![
        button(text(fl!("credentials-new")))
            .style(theme::pill)
            .on_press(Message::NewPressed),
        scrollable(column(entries).spacing(2)),
    ]
    .spacing(10)
    .width(Length::Fixed(LIST_WIDTH));

    let form = match state.draft() {
        Some(draft) => view_form(draft),
        None => text(fl!("credentials-empty"))
            .style(theme::secondary_text)
            .into(),
    };

    column![header, row![list, form].spacing(theme::PANEL_PADDING)]
        .spacing(12)
        .into()
}

fn view_form(draft: &Draft) -> Element<'_, Message> {
    let mut fields = column![
        labelled(
            fl!("credentials-name"),
            theme::input("", &draft.name).on_input(Message::NameChanged),
        ),
        labelled(
            fl!("credentials-username"),
            theme::input("", &draft.username).on_input(Message::UsernameChanged),
        ),
        labelled(
            fl!("credentials-domain"),
            theme::input("", &draft.domain).on_input(Message::DomainChanged),
        ),
        labelled(
            fl!("credentials-kind"),
            theme::pick(KINDS, Some(KindChoice(draft.kind)), Message::KindPicked),
        ),
    ]
    .spacing(10)
    .width(Length::Fill);

    if draft.kind == CredentialKind::PrivateKey {
        fields = fields.push(labelled(
            fl!("credentials-key-file"),
            theme::input("", &draft.key_file).on_input(Message::KeyFileChanged),
        ));
    }

    if draft.kind != CredentialKind::Ask {
        let label = match draft.kind {
            CredentialKind::PrivateKey => fl!("credentials-passphrase"),
            _ => fl!("credentials-password"),
        };
        let input = theme::input(&fl!("credentials-secret-keep"), &draft.secret)
            .secure(true)
            .on_input(|typed| Message::SecretChanged(Hidden(typed)));
        fields = fields.push(labelled(label, input));
    }

    let buttons = row![
        button(text(fl!("credentials-save")))
            .style(theme::primary)
            .on_press(Message::SavePressed),
        button(text(fl!("credentials-delete")))
            .style(theme::danger)
            .on_press(Message::DeletePressed),
    ]
    .spacing(8);

    fields.push(buttons).into()
}

fn labelled<'a>(label: String, field: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![
        text(label)
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        field.into()
    ]
    .spacing(2)
    .into()
}

#[cfg(test)]
mod tests {
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
}
