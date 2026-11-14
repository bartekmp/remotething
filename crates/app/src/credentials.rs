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
#[path = "credentials_tests.rs"]
mod tests;
