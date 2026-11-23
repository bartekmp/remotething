//! What the window does for the credentials screen: add, save and delete a
//! credential, and keep its secret in the secret store in step.

use std::sync::Arc;

use iced::Task;
use remotething_core::{Credential, CredentialKind};
use remotething_secrets::SecretStore;

use crate::app::{Message, State};
use crate::credentials::SecretAction;
use crate::i18n::fl;

impl State {
    pub(crate) fn add_credential(&mut self) {
        let name = fl!("credentials-new-name");
        let credential = Credential {
            id: self.store.new_credential_id(&name),
            name,
            username: None,
            domain: None,
            kind: CredentialKind::Password,
            key_file: None,
        };
        let id = credential.id.clone();

        let mut updated = self.store.clone();
        updated.credentials.push(credential);
        if self.commit(updated) {
            self.credentials.select(&self.store, Some(id));
        }
    }

    /// Saves the description to the file, then the secret to the secret store.
    pub(crate) fn save_credential(&mut self) -> Task<Message> {
        let (Some(id), Some(draft)) = (self.credentials.selected(), self.credentials.draft())
        else {
            return Task::none();
        };
        let id = id.to_string();

        match draft.apply(&self.store, &id) {
            Ok(updated) => {
                if !self.commit(updated) {
                    return Task::none();
                }
                let action = self.credentials.take_secret_action();
                self.secret_task(id, action)
            }
            Err(reason) => {
                self.error = Some(reason);
                Task::none()
            }
        }
    }

    /// Asks before removing the selected credential, unless something still uses it.
    pub(crate) fn delete_credential(&mut self) -> Task<Message> {
        let Some(id) = self.credentials.selected().map(str::to_string) else {
            return Task::none();
        };
        // A credential in use is refused at once, with the reason, instead of asked about.
        if let Err(error) = self.store.clone().remove_credential(&id) {
            self.error = Some(error.to_string());
            return Task::none();
        }
        let name = self
            .store
            .credentials
            .iter()
            .find(|credential| credential.id == id)
            .map(|credential| credential.name.clone())
            .unwrap_or_default();
        self.removal = Some(crate::removal::Question::new(
            crate::removal::Subject::Credential(id),
            name,
        ));
        Task::none()
    }

    /// Removes a credential and its secret, once the user said yes.
    pub(crate) fn remove_credential(&mut self, id: &str) -> Task<Message> {
        let id = id.to_string();
        let mut updated = self.store.clone();
        if let Err(error) = updated.remove_credential(&id) {
            self.error = Some(error.to_string());
            return Task::none();
        }
        if !self.commit(updated) {
            return Task::none();
        }
        self.credentials.select(&self.store, None);
        self.secret_task(id, SecretAction::Delete)
    }

    /// Runs the secret store call off the UI thread; it can wait on an unlock prompt.
    pub(crate) fn secret_task(&self, credential_id: String, action: SecretAction) -> Task<Message> {
        if matches!(action, SecretAction::Keep) {
            return Task::none();
        }
        let secrets = Arc::clone(&self.secrets);
        Task::perform(
            async move { apply_secret(secrets.as_ref(), &credential_id, action) },
            Message::SecretStored,
        )
    }
}

/// Carries out what the credentials screen decided about the stored secret.
pub(crate) fn apply_secret(
    secrets: &dyn SecretStore,
    credential_id: &str,
    action: SecretAction,
) -> Result<(), String> {
    let result = match action {
        SecretAction::Keep => Ok(()),
        SecretAction::Set(secret) => secrets.set(credential_id, &secret),
        SecretAction::Delete => secrets.delete(credential_id),
    };
    result.map_err(|error| error.to_string())
}
