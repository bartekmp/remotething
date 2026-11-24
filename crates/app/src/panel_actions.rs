//! What the window does when one of its panels asks for something: the palette,
//! the properties, the credentials and the master-password prompt.

use iced::Task;

use crate::app::{Message, State, Unlocked};
use crate::credentials;
use crate::i18n::fl;
use crate::input;
use crate::palette;
use crate::properties;
use crate::vault;

pub(crate) fn on_palette(state: &mut State, message: palette::Message) -> Task<Message> {
    let Some(open) = &mut state.palette else {
        return Task::none();
    };
    let entries = palette::entries(&state.store, state.recent.ids(), open.query());
    match palette::update(open, message, &entries) {
        Some(palette::Event::Chosen(item)) => {
            state.palette = None;
            return input::open_palette_item(state, item);
        }
        Some(palette::Event::Close) => state.palette = None,
        None => {}
    }
    Task::none()
}

pub(crate) fn on_properties(state: &mut State, message: properties::Message) -> Task<Message> {
    let Some(draft) = &mut state.draft else {
        return Task::none();
    };
    let selected = state.tree.selected().cloned();
    match (properties::update(draft, message), selected) {
        (Some(properties::Event::Save), _) => return state.save_draft(),
        (Some(properties::Event::Delete), Some(item)) => state.delete(&item),
        _ => {}
    }
    Task::none()
}

pub(crate) fn on_credentials(state: &mut State, message: credentials::Message) -> Task<Message> {
    match credentials::update(&mut state.credentials, &state.store, message) {
        Some(credentials::Event::New) => state.add_credential(),
        Some(credentials::Event::Save) => return state.save_credential(),
        Some(credentials::Event::Delete) => return state.delete_credential(),
        None => {}
    }
    Task::none()
}

pub(crate) fn on_vault(state: &mut State, message: vault::Message) -> Task<Message> {
    let Some(prompt) = &mut state.vault else {
        return Task::none();
    };
    match vault::update(prompt, message) {
        Some(vault::Event::Open(request)) => {
            // Key derivation is slow on purpose; keep it off the UI thread.
            return Task::perform(
                async move { vault::open(request).map(Unlocked) },
                Message::VaultOpened,
            );
        }
        Some(vault::Event::Skip) => {
            state.vault = None;
            state.error = Some(fl!("vault-skipped"));
        }
        None => {}
    }
    Task::none()
}

/// The prompt's answer: the unlocked store replaces the empty one, or the prompt says why not.
pub(crate) fn on_vault_opened(state: &mut State, result: Result<Unlocked, String>) {
    match result {
        Ok(unlocked) => {
            state.secrets = unlocked.0;
            state.vault = None;
            state.error = None;
        }
        Err(reason) => {
            if let Some(prompt) = &mut state.vault {
                prompt.fail(reason);
            }
        }
    }
}
