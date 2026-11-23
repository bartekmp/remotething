//! What the window does to folders and connections: add, save, remove, rename, move.

use remotething_core::{
    Connection, Credential, CredentialKind, Direction, Folder, Protocol, QuickTarget, Settings,
    Store,
};

use iced::Task;

use crate::app::{Centre, Message, State};
use crate::credentials::SecretAction;
use crate::folder_removal;
use crate::i18n::fl;
use crate::import_preview;
use crate::organise;
use crate::session;
use crate::tree::{DropTarget, Selection};

impl State {
    /// New items go into the selected folder, or beside the selected connection.
    pub(crate) fn folder_for_new_item(&self) -> Option<String> {
        match self.tree.selected()? {
            Selection::Folder(id) => Some(id.clone()),
            Selection::Connection(id) => self
                .store
                .connections
                .iter()
                .find(|connection| &connection.id == id)
                .and_then(|connection| connection.parent.clone()),
        }
    }

    pub(crate) fn add_folder(&mut self, parent: Option<String>) {
        let name = fl!("tree-new-folder-name");
        let folder = Folder {
            id: self.store.new_folder_id(&name),
            name,
            parent,
            icon: None,
            settings: Settings::default(),
        };
        let selection = Selection::Folder(folder.id.clone());
        if let Some(parent) = &folder.parent {
            self.tree.expand(parent);
        }

        let mut updated = self.store.clone();
        updated.folders.push(folder);
        if self.commit(updated) {
            self.select(Some(selection));
            self.properties_open = true;
        }
    }

    pub(crate) fn add_connection(&mut self, parent: Option<String>) {
        let name = fl!("tree-new-connection-name");
        let connection = Connection {
            id: self.store.new_connection_id(&name),
            name,
            parent,
            protocol: Protocol::Ssh,
            host: fl!("tree-new-connection-host"),
            icon: None,
            settings: Settings::default(),
        };
        let selection = Selection::Connection(connection.id.clone());
        if let Some(parent) = &connection.parent {
            self.tree.expand(parent);
        }

        let mut updated = self.store.clone();
        updated.connections.push(connection);
        if self.commit(updated) {
            self.select(Some(selection));
            self.properties_open = true;
        }
    }

    /// Adds a copy of a connection right after it, selects it and opens it for editing.
    pub(crate) fn duplicate_connection(&mut self, id: &str) {
        let Some(original) = self.store.connections.iter().find(|c| c.id == id) else {
            return;
        };
        let name = fl!("tree-copy-name", name = original.name.as_str());
        let mut updated = self.store.clone();
        let Ok(copy_id) = updated.duplicate_connection(id, name) else {
            return;
        };
        if self.commit(updated) {
            self.select(Some(Selection::Connection(copy_id)));
            self.properties_open = true;
        }
    }

    pub(crate) fn save_draft(&mut self) -> Task<Message> {
        let (Some(draft), Some(selection)) = (&self.draft, self.tree.selected()) else {
            return Task::none();
        };
        match draft.apply_with_password(&self.store, selection) {
            Ok(saved) => {
                // A saved edit is finished; a refused one stays open to be corrected.
                if !self.commit(saved.store) {
                    return Task::none();
                }
                self.properties_open = false;
                // The password goes to the secret store off the UI thread; it can wait on an unlock prompt.
                match saved.secret {
                    Some((id, secret)) => self.secret_task(id, SecretAction::Set(secret)),
                    None => Task::none(),
                }
            }
            Err(reason) => {
                self.error = Some(reason);
                Task::none()
            }
        }
    }

    /// Moves an item one place up or down among those in its folder.
    pub(crate) fn shift(&mut self, item: &Selection, direction: Direction) {
        let mut updated = self.store.clone();
        let result = match item {
            Selection::Folder(id) => updated.shift_folder(id, direction),
            Selection::Connection(id) => updated.shift_connection(id, direction),
        };
        match result {
            Ok(()) => {
                self.commit(updated);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(crate) fn delete(&mut self, item: &Selection) {
        if let Selection::Folder(id) = item {
            let inside = self.store.count_inside(id);
            if inside != (0, 0) {
                // Content goes with the folder, so the user decides first.
                let name = organise::name_of(&self.store, item).unwrap_or_default();
                self.removal = Some(folder_removal::Question::new(id.clone(), name, inside));
                return;
            }
        }
        let mut updated = self.store.clone();
        let removed = match item {
            Selection::Folder(id) => updated.remove_folder(id),
            Selection::Connection(id) => updated.remove_connection(id),
        };
        self.finish_removal(updated, removed);
    }

    /// The answer to the question about removing a folder with content.
    pub(crate) fn answer_removal(&mut self, answer: folder_removal::Message) {
        let Some(question) = self.removal.take() else {
            return;
        };
        if answer == folder_removal::Message::Confirmed {
            let mut updated = self.store.clone();
            let removed = updated.remove_folder_tree(&question.folder_id);
            self.finish_removal(updated, removed);
        }
    }

    fn finish_removal(&mut self, updated: Store, removed: remotething_core::Result<()>) {
        match removed {
            Ok(()) => {
                if self.commit(updated) {
                    self.select(None);
                    self.properties_open = false;
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// Opens a tab for the connection.
    pub(crate) fn open(&mut self, connection_id: &str) -> Task<Message> {
        // The tree only offers connections from this store, so this resolves.
        let Ok(connection) = self.store.resolve(connection_id) else {
            return Task::none();
        };
        let context = session::Context {
            store: &self.store,
            secrets: &self.secrets,
            known_hosts: &mut self.known_hosts,
        };
        self.tabs
            .open(connection, &context, &mut self.recent)
            .map(Message::Tabs)
    }

    /// Opens a tab with the user's own shell on this machine.
    pub(crate) fn open_local_terminal(&mut self) -> Task<Message> {
        let context = session::Context {
            store: &self.store,
            secrets: &self.secrets,
            known_hosts: &mut self.known_hosts,
        };
        self.tabs.open_local_terminal(&context).map(Message::Tabs)
    }

    pub(crate) fn rename(&mut self, item: &Selection, name: &str) {
        match organise::renamed(&self.store, item, name) {
            Ok(updated) => self.commit_and_reload(updated),
            Err(reason) => self.error = Some(reason),
        }
    }

    pub(crate) fn set_icon(&mut self, item: &Selection, icon: Option<&str>) {
        let updated = organise::with_icon(&self.store, item, icon);
        self.commit_and_reload(updated);
    }

    /// Carries out a drop in the tree. A folder that receives something is unfolded.
    pub(crate) fn move_item(&mut self, source: &Selection, target: &DropTarget) {
        match organise::moved(&self.store, source, target) {
            Ok(updated) => {
                if let DropTarget::Row(Selection::Folder(id)) = target {
                    self.tree.expand(id);
                }
                self.commit_and_reload(updated);
            }
            Err(reason) => self.error = Some(reason),
        }
    }

    /// Puts a connection that was typed into a new tab into the tree and opens
    /// its properties, filled in with what was typed.
    ///
    /// A typed user name becomes an "ask every time" credential, because the
    /// user belongs to a credential and not to the connection.
    pub(crate) fn save_typed_connection(&mut self, tab: u64, target: &QuickTarget) {
        let mut updated = self.store.clone();
        let credential = target.username.as_ref().map(|username| {
            let name = target.label();
            let id = updated.new_credential_id(&name);
            updated.credentials.push(Credential {
                id: id.clone(),
                name,
                username: Some(username.clone()),
                domain: None,
                kind: CredentialKind::Ask,
                key_file: None,
            });
            id
        });
        // A local shell with no name typed is the user's own.
        let name = if target.protocol == Protocol::Local {
            crate::local_shell::title(&target.host)
        } else {
            target.host.clone()
        };
        let connection = Connection {
            id: updated.new_connection_id(&name),
            name,
            parent: None,
            protocol: target.protocol,
            host: target.host.clone(),
            icon: None,
            settings: Settings {
                port: target.port,
                credential,
                ..Settings::default()
            },
        };
        let (id, name) = (connection.id.clone(), connection.name.clone());
        updated.connections.push(connection);

        if self.commit(updated) {
            self.tabs.mark_saved(tab, &id, &name);
            self.recent.touch(&id);
            self.select(Some(Selection::Connection(id)));
            self.properties_open = true;
        }
    }

    /// Adds the chosen PuTTY sessions to the tree and goes back to the sessions.
    pub(crate) fn apply_putty(&mut self, plan: &remotething_core::ImportPlan, names: &[String]) {
        let mut updated = self.store.clone();
        match updated.import_putty(plan, names) {
            Ok(added) => {
                if self.commit(updated) {
                    self.notice = Some(fl!("putty-done", count = added));
                    self.settings_screen = crate::app::Centre::Settings;
                    self.tabs.close_page(crate::tabs::Page::Settings);
                }
            }
            Err(error) => self.error = Some(format!("{} {error}", fl!("import-failed"))),
        }
    }

    /// Adds the ticked WSL distributions to the `WSL` folder.
    pub(crate) fn apply_wsl(&mut self, plan: &remotething_core::WslPlan, names: &[String]) {
        let mut updated = self.store.clone();
        match updated.import_wsl(plan, names) {
            Ok(done) => {
                if self.commit(updated) {
                    self.notice = Some(fl!(
                        "wsl-done",
                        added = done.added,
                        existing = done.already_there
                    ));
                    self.settings_screen = crate::app::Centre::Settings;
                    self.tabs.close_page(crate::tabs::Page::Settings);
                }
            }
            Err(error) => self.error = Some(format!("{} {error}", fl!("import-failed"))),
        }
    }

    /// Writes the whole tree to an export file. Secrets are not part of it.
    pub(crate) fn export_to(&mut self, file: &std::path::Path) {
        let written = self
            .store
            .to_export()
            .map_err(|error| error.to_string())
            .and_then(|text| std::fs::write(file, text).map_err(|error| error.to_string()));
        match written {
            Ok(()) => {
                self.error = None;
                self.notice = Some(fl!("export-done", file = file.display().to_string()));
            }
            Err(reason) => self.error = Some(format!("{} {reason}", fl!("export-failed"))),
        }
    }

    /// Reads an export file and shows what is in it, to choose from.
    pub(crate) fn preview_import(&mut self, file: &std::path::Path) {
        let read = std::fs::read_to_string(file)
            .map_err(|error| error.to_string())
            .and_then(|text| Store::from_export(&text).map_err(|error| error.to_string()));
        match read {
            Ok(incoming) => {
                self.error = None;
                self.import = Some(import_preview::State::new(incoming));
                self.open_settings(Centre::Import);
            }
            Err(reason) => self.error = Some(format!("{} {reason}", fl!("import-failed"))),
        }
    }

    /// Adds the chosen tree. Existing items are never changed.
    pub(crate) fn import_tree(&mut self, incoming: Store) {
        let mut updated = self.store.clone();
        match updated.merge(incoming) {
            Ok(summary) => {
                if self.commit(updated) {
                    self.notice = Some(fl!(
                        "import-done",
                        connections = summary.connections,
                        folders = summary.folders,
                        credentials = summary.credentials,
                        renamed = summary.renamed
                    ));
                }
            }
            Err(error) => self.error = Some(format!("{} {error}", fl!("import-failed"))),
        }
    }

    /// Saves, then shows the new values in the properties panel.
    fn commit_and_reload(&mut self, updated: Store) {
        if self.commit(updated) {
            let selection = self.tree.selected().cloned();
            self.select(selection);
        }
    }
}

#[cfg(test)]
#[path = "items_tests.rs"]
mod tests;
