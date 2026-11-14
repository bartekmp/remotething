//! The window: tree on the left, tabs in the centre, properties on the right.
//! The credentials and settings screens take the centre while they are open.

use std::path::PathBuf;
use std::sync::Arc;

use iced::theme::Mode;
use iced::widget::operation;
use iced::{Event, Point, Size, Subscription, Task, Theme, keyboard, mouse, window};
use remotething_core::{Credential, CredentialKind, Store};
use remotething_secrets::SecretStore;

use crate::credentials::{self, SecretAction};
use crate::i18n::fl;
use crate::menu;
use crate::organise;
use crate::preferences::{self, Preferences, ThemeChoice};
use crate::properties::{self, Draft};
use crate::settings;
use crate::tabs;
use crate::theme as look;
use crate::tree::{self, Selection};
use crate::vault;

/// The window size iced starts with, until the first size event arrives.
const INITIAL_WINDOW: Size = Size::new(1024.0, 768.0);

pub struct State {
    /// Where the connections file is read from and saved to.
    pub(crate) path: PathBuf,
    pub(crate) store: Store,
    /// The last load, save or edit problem, shown at the top of the window.
    pub(crate) error: Option<String>,
    pub(crate) tree: tree::State,
    pub(crate) tabs: tabs::State,
    /// The properties being edited for the selected item.
    pub(crate) draft: Option<Draft>,
    pub(crate) credentials: credentials::State,
    pub(crate) centre: Centre,
    pub(crate) secrets: Arc<dyn SecretStore>,
    pub(crate) preferences: Preferences,
    /// The master-password prompt, while the encrypted passwords file is locked.
    pub(crate) vault: Option<vault::State>,
    /// Whether the desktop is light or dark, for the "follow the system" theme.
    pub(crate) system_mode: Mode,
    /// The context menu of the tree, while it is open.
    pub(crate) menu: Option<menu::State>,
    /// Where the pointer is, in window coordinates.
    pub(crate) pointer: Point,
    pub(crate) window: Size,
}

/// An unlocked secret store on its way from the worker thread to the window.
#[derive(Clone)]
pub struct Unlocked(Arc<dyn SecretStore>);

impl std::fmt::Debug for Unlocked {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Unlocked")
    }
}

/// What fills the middle of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Centre {
    Sessions,
    Credentials,
    Settings,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tree(tree::Message),
    Tabs(tabs::Message),
    Properties(properties::Message),
    Credentials(credentials::Message),
    /// The secret store finished a save or delete.
    SecretStored(Result<(), String>),
    Settings(settings::Message),
    Vault(vault::Message),
    /// Creating or unlocking the passwords file finished.
    VaultOpened(Result<Unlocked, String>),
    /// The desktop switched between light and dark.
    SystemModeChanged(Mode),
    /// The collapse or expand button of the connection tree.
    TreeCollapseToggled,
    /// The collapse or expand button of the properties panel.
    PropertiesCollapseToggled,
    Menu(menu::Message),
    PointerMoved(Point),
    /// The left mouse button went up, anywhere in the window.
    PointerReleased,
    EscapePressed,
    WindowResized(Size),
}

impl State {
    /// `notice` is a start-up message to show, such as a missing keyring.
    pub fn new(
        path: PathBuf,
        loaded: Result<Store, String>,
        secrets: Arc<dyn SecretStore>,
        notice: Option<String>,
    ) -> Self {
        let (store, error) = match loaded {
            Ok(store) => (store, notice),
            Err(reason) => (
                Store::default(),
                Some(format!("{} {reason}", fl!("error-load"))),
            ),
        };
        Self {
            path,
            store,
            error,
            tree: tree::State::default(),
            tabs: tabs::State::default(),
            draft: None,
            credentials: credentials::State::default(),
            centre: Centre::Sessions,
            secrets,
            preferences: Preferences::default(),
            vault: None,
            system_mode: Mode::None,
            menu: None,
            pointer: Point::ORIGIN,
            window: INITIAL_WINDOW,
        }
    }

    /// Starts by asking for the master password of the encrypted passwords file.
    /// Until it is unlocked, secrets go to the store given to `new`.
    pub fn with_vault_prompt(mut self, vault_file: PathBuf, keyring_problem: String) -> Self {
        self.vault = Some(vault::State::new(vault_file, keyring_problem));
        self
    }

    /// Starts with the preferences read from the settings file.
    pub fn with_preferences(mut self, preferences: Preferences) -> Self {
        self.preferences = preferences;
        self
    }

    fn save_preferences(&mut self) {
        let path = preferences::path_beside(&self.path);
        match preferences::save(&self.preferences, &path) {
            Ok(()) => self.error = None,
            Err(reason) => {
                self.error = Some(format!("{} {reason}", fl!("error-settings-save")));
            }
        }
    }

    fn add_credential(&mut self) {
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
    fn save_credential(&mut self) -> Task<Message> {
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

    fn delete_credential(&mut self) -> Task<Message> {
        let Some(id) = self.credentials.selected().map(str::to_string) else {
            return Task::none();
        };
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
    fn secret_task(&self, credential_id: String, action: SecretAction) -> Task<Message> {
        if matches!(action, SecretAction::Keep) {
            return Task::none();
        }
        let secrets = Arc::clone(&self.secrets);
        Task::perform(
            async move { apply_secret(secrets.as_ref(), &credential_id, action) },
            Message::SecretStored,
        )
    }

    /// Saves a changed store; the one in memory is replaced only if that works.
    pub(crate) fn commit(&mut self, updated: Store) -> bool {
        let saved = save(&updated, &self.path);
        match saved {
            Ok(()) => {
                self.store = updated;
                self.tree.rows_changed();
                self.error = None;
                true
            }
            Err(reason) => {
                self.error = Some(format!("{} {reason}", fl!("error-save")));
                false
            }
        }
    }

    pub(crate) fn select(&mut self, selection: Option<Selection>) {
        self.draft = selection
            .as_ref()
            .and_then(|selection| Draft::load(&self.store, selection));
        self.tree.select(selection);
    }
}

/// Validates and writes the store, creating the directory on first use.
fn save(store: &Store, path: &std::path::Path) -> Result<(), String> {
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    }
    store.save(path).map_err(|error| error.to_string())
}

/// Carries out what the credentials screen decided about the stored secret.
fn apply_secret(
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

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Tree(message) => {
            if let Some(event) = tree::update(&mut state.tree, message) {
                return on_tree_event(state, event);
            }
        }
        Message::PointerReleased => {
            if let Some(event) = state.tree.release() {
                return on_tree_event(state, event);
            }
        }
        Message::Menu(message) => return on_menu(state, message),
        Message::PointerMoved(position) => state.pointer = position,
        Message::WindowResized(size) => state.window = size,
        Message::EscapePressed => {
            state.menu = None;
            state.tree.cancel();
        }
        Message::Tabs(message) => tabs::update(&mut state.tabs, message),
        Message::Properties(message) => {
            let Some(draft) = &mut state.draft else {
                return Task::none();
            };
            let selected = state.tree.selected().cloned();
            match (properties::update(draft, message), selected) {
                (Some(properties::Event::Save), _) => state.save_draft(),
                (Some(properties::Event::Delete), Some(item)) => state.delete(&item),
                _ => {}
            }
        }
        Message::Credentials(message) => {
            match credentials::update(&mut state.credentials, &state.store, message) {
                Some(credentials::Event::New) => state.add_credential(),
                Some(credentials::Event::Save) => return state.save_credential(),
                Some(credentials::Event::Delete) => return state.delete_credential(),
                Some(credentials::Event::Close) => state.centre = Centre::Sessions,
                None => {}
            }
        }
        Message::Vault(message) => {
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
        }
        Message::SystemModeChanged(mode) => state.system_mode = mode,
        Message::TreeCollapseToggled => {
            state.preferences.tree_collapsed = !state.preferences.tree_collapsed;
            state.save_preferences();
        }
        Message::PropertiesCollapseToggled => {
            state.preferences.properties_collapsed = !state.preferences.properties_collapsed;
            state.save_preferences();
        }
        Message::VaultOpened(Ok(unlocked)) => {
            state.secrets = unlocked.0;
            state.vault = None;
            state.error = None;
        }
        Message::VaultOpened(Err(reason)) => {
            if let Some(prompt) = &mut state.vault {
                prompt.fail(reason);
            }
        }
        Message::Settings(message) => match settings::update(&mut state.preferences, message) {
            settings::Event::Changed => state.save_preferences(),
            settings::Event::Close => state.centre = Centre::Sessions,
        },
        Message::SecretStored(Ok(())) => {}
        Message::SecretStored(Err(reason)) => {
            state.error = Some(format!("{} {reason}", fl!("error-secret")));
        }
    }
    Task::none()
}

fn on_tree_event(state: &mut State, event: tree::Event) -> Task<Message> {
    match event {
        tree::Event::SelectionChanged => {
            let selection = state.tree.selected().cloned();
            state.select(selection);
        }
        tree::Event::Open(connection_id) => state.open(&connection_id),
        tree::Event::NewFolder => state.add_folder(state.folder_for_new_item()),
        tree::Event::NewConnection => state.add_connection(state.folder_for_new_item()),
        tree::Event::ShowCredentials => state.centre = Centre::Credentials,
        tree::Event::ShowSettings => state.centre = Centre::Settings,
        tree::Event::Menu(target) => {
            if target.is_some() {
                state.select(target.clone());
            }
            state.menu = Some(menu::State::new(target, state.pointer));
        }
        tree::Event::Renamed(item, name) => state.rename(&item, &name),
        tree::Event::Moved { source, target } => state.move_item(&source, &target),
    }
    Task::none()
}

/// Carries out an entry of the context menu on what was right-clicked.
fn on_menu(state: &mut State, message: menu::Message) -> Task<Message> {
    if message == menu::Message::ChangeIcon {
        if let Some(menu) = &mut state.menu {
            menu.show_icons();
        }
        return Task::none();
    }
    let Some(menu) = state.menu.take() else {
        return Task::none();
    };
    let target = menu.target();
    // New items go inside a right-clicked folder, or to the top level.
    let folder = match target {
        Some(Selection::Folder(id)) => Some(id.clone()),
        _ => None,
    };

    match (message, target) {
        (menu::Message::NewFolder, _) => state.add_folder(folder),
        (menu::Message::NewConnection, _) => state.add_connection(folder),
        (menu::Message::Connect, Some(Selection::Connection(id))) => state.open(id),
        (menu::Message::Rename, Some(item)) => {
            if let Some(name) = organise::name_of(&state.store, item) {
                state.tree.start_rename(item.clone(), name);
                return operation::focus(tree::RENAME_INPUT)
                    .chain(operation::select_all(tree::RENAME_INPUT));
            }
        }
        (menu::Message::IconPicked(icon), Some(item)) => state.set_icon(item, icon),
        (menu::Message::Remove, Some(item)) => state.delete(item),
        _ => {}
    }
    Task::none()
}

pub fn title(_state: &State) -> String {
    fl!("app-title")
}

/// The chosen theme, or the one matching the desktop for "follow the system".
pub fn theme(state: &State) -> Theme {
    let mode = match state.preferences.theme {
        ThemeChoice::System => state.system_mode,
        ThemeChoice::Light => Mode::Light,
        ThemeChoice::Dark => Mode::Dark,
    };
    look::for_mode(mode)
}

/// Keeps the theme in step when the desktop switches between light and dark.
pub fn subscription(_state: &State) -> Subscription<Message> {
    Subscription::batch([
        iced::system::theme_changes().map(Message::SystemModeChanged),
        iced::event::listen_with(window_event),
    ])
}

/// The window-wide events behind the context menu, dragging and Escape.
fn window_event(
    event: Event,
    _status: iced::event::Status,
    _window: window::Id,
) -> Option<Message> {
    match event {
        Event::Mouse(mouse::Event::CursorMoved { position }) => {
            Some(Message::PointerMoved(position))
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(Message::PointerReleased)
        }
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        }) => Some(Message::EscapePressed),
        Event::Window(window::Event::Opened { size, .. })
        | Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use remotething_secrets::{MemoryStore, Secret};

    const ONE_CONNECTION: &str = r#"
        [[folder]]
        id = "prod"
        name = "Production"

        [[connection]]
        id = "web-01"
        name = "web-01"
        parent = "prod"
        protocol = "ssh"
        host = "web-01.example.net"
    "#;

    /// A window state backed by a real file in a temporary directory.
    fn state(directory: &tempfile::TempDir, content: &str) -> State {
        let path = directory.path().join("nested").join("connections.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        let store = Store::load(&path).unwrap();
        State::new(path, Ok(store), Arc::new(MemoryStore::new()), None)
    }

    // The returned tasks need a running app; these tests check the synchronous part.
    fn tree(state: &mut State, message: tree::Message) {
        let _ = update(state, Message::Tree(message));
    }

    fn properties(state: &mut State, message: properties::Message) {
        let _ = update(state, Message::Properties(message));
    }

    fn credentials(state: &mut State, message: credentials::Message) {
        let _ = update(state, Message::Credentials(message));
    }

    fn on_disk(state: &State) -> Store {
        Store::load(&state.path).unwrap()
    }

    #[test]
    fn opening_a_connection_in_the_tree_opens_a_tab() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);

        tree(
            &mut state,
            tree::Message::ConnectionOpened("web-01".to_string()),
        );

        assert_eq!(state.tabs.open_ids(), ["web-01"]);
    }

    #[test]
    fn selecting_a_connection_loads_its_properties_without_opening_a_tab() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);

        tree(
            &mut state,
            tree::Message::ConnectionSelected("web-01".to_string()),
        );

        assert!(state.tabs.open_ids().is_empty());
        assert_eq!(state.draft.as_ref().unwrap().host, "web-01.example.net");
    }

    #[test]
    fn a_new_connection_lands_in_the_selected_folder_and_on_disk() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);
        tree(
            &mut state,
            tree::Message::FolderSelected("prod".to_string()),
        );

        tree(&mut state, tree::Message::NewConnectionPressed);

        let saved = on_disk(&state);
        let added = saved.connections.last().unwrap();
        assert_eq!(added.id, "new-connection");
        assert_eq!(added.parent.as_deref(), Some("prod"));
        // The new item is selected, ready to be renamed.
        assert_eq!(
            state.tree.selected(),
            Some(&Selection::Connection(added.id.clone()))
        );
        assert!(state.draft.is_some());
    }

    #[test]
    fn a_new_folder_with_nothing_selected_goes_to_the_top_level() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, "");

        tree(&mut state, tree::Message::NewFolderPressed);
        tree(
            &mut state,
            tree::Message::FolderSelected("new-folder".to_string()),
        );
        tree(&mut state, tree::Message::NewFolderPressed);

        let saved = on_disk(&state);
        assert_eq!(saved.folders[0].parent, None);
        assert_eq!(saved.folders[1].id, "new-folder-2");
        assert_eq!(saved.folders[1].parent.as_deref(), Some("new-folder"));
    }

    #[test]
    fn saving_an_edit_writes_it_to_disk() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);
        tree(
            &mut state,
            tree::Message::ConnectionSelected("web-01".to_string()),
        );

        properties(
            &mut state,
            properties::Message::HostChanged("10.0.0.5".to_string()),
        );
        properties(&mut state, properties::Message::SavePressed);

        assert_eq!(on_disk(&state).connections[0].host, "10.0.0.5");
        assert_eq!(state.error, None);
    }

    #[test]
    fn a_refused_edit_changes_nothing_and_says_why() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);
        tree(
            &mut state,
            tree::Message::ConnectionSelected("web-01".to_string()),
        );

        properties(
            &mut state,
            properties::Message::PortChanged("not a port".to_string()),
        );
        properties(&mut state, properties::Message::SavePressed);

        assert!(state.error.is_some());
        assert_eq!(on_disk(&state).connections[0].settings.port, None);
    }

    #[test]
    fn deleting_a_connection_removes_it_and_clears_the_panel() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);
        tree(
            &mut state,
            tree::Message::ConnectionSelected("web-01".to_string()),
        );

        properties(&mut state, properties::Message::DeletePressed);

        assert!(on_disk(&state).connections.is_empty());
        assert_eq!(state.tree.selected(), None);
        assert_eq!(state.draft, None);
    }

    #[test]
    fn a_folder_with_content_is_not_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);
        tree(
            &mut state,
            tree::Message::FolderSelected("prod".to_string()),
        );

        properties(&mut state, properties::Message::DeletePressed);

        assert_eq!(on_disk(&state).folders.len(), 1);
        assert!(state.error.as_deref().unwrap().contains("not empty"));
    }

    #[test]
    fn the_first_save_creates_the_directory() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fresh").join("connections.toml");
        let secrets = Arc::new(MemoryStore::new());
        let mut state = State::new(path.clone(), Ok(Store::default()), secrets, None);

        tree(&mut state, tree::Message::NewFolderPressed);

        assert!(path.exists());
    }

    #[test]
    fn a_load_failure_starts_with_an_empty_tree_and_shows_the_reason() {
        let secrets = Arc::new(MemoryStore::new());
        let broken = Err("file is broken".to_string());
        let state = State::new(PathBuf::from("unused"), broken, secrets, None);

        assert_eq!(state.store, Store::default());
        assert!(state.error.as_deref().unwrap().contains("file is broken"));
    }

    const WITH_CREDENTIAL: &str = r#"
        [[credential]]
        id = "cred-ops"
        name = "Ops team"
        kind = "password"

        [[connection]]
        id = "web-01"
        name = "web-01"
        protocol = "ssh"
        host = "web-01.example.net"
        credential = "cred-ops"
    "#;

    #[test]
    fn the_credentials_button_opens_the_screen_and_close_returns() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);

        tree(&mut state, tree::Message::CredentialsPressed);
        assert_eq!(state.centre, Centre::Credentials);

        credentials(&mut state, credentials::Message::ClosePressed);
        assert_eq!(state.centre, Centre::Sessions);
    }

    #[test]
    fn a_new_credential_is_saved_and_selected() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, "");

        credentials(&mut state, credentials::Message::NewPressed);

        let saved = on_disk(&state);
        assert_eq!(saved.credentials[0].id, "new-credential");
        assert_eq!(state.credentials.selected(), Some("new-credential"));
    }

    #[test]
    fn saving_a_credential_writes_the_description_and_keeps_the_secret_out_of_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, WITH_CREDENTIAL);
        credentials(
            &mut state,
            credentials::Message::Selected("cred-ops".to_string()),
        );
        credentials(
            &mut state,
            credentials::Message::UsernameChanged("ops".to_string()),
        );
        let typed = credentials::Hidden("hunter2".to_string());
        credentials(&mut state, credentials::Message::SecretChanged(typed));

        credentials(&mut state, credentials::Message::SavePressed);

        assert_eq!(
            on_disk(&state).credentials[0].username.as_deref(),
            Some("ops")
        );
        let file_text = std::fs::read_to_string(&state.path).unwrap();
        assert!(!file_text.contains("hunter2"));
        // The typed secret has left the form on its way to the store.
        assert_eq!(state.credentials.draft().unwrap().secret, "");
    }

    #[test]
    fn a_credential_in_use_is_not_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, WITH_CREDENTIAL);
        credentials(
            &mut state,
            credentials::Message::Selected("cred-ops".to_string()),
        );

        credentials(&mut state, credentials::Message::DeletePressed);

        assert_eq!(on_disk(&state).credentials.len(), 1);
        assert!(state.error.as_deref().unwrap().contains("web-01"));
    }

    #[test]
    fn secret_actions_reach_the_store() {
        let secrets = MemoryStore::new();
        let set = SecretAction::Set(Secret::new("hunter2".to_string()));

        apply_secret(&secrets, "cred-ops", set).unwrap();
        let stored = secrets.get("cred-ops").unwrap().unwrap();
        assert_eq!(stored.expose(), "hunter2");

        apply_secret(&secrets, "cred-ops", SecretAction::Keep).unwrap();
        assert!(secrets.get("cred-ops").unwrap().is_some());

        apply_secret(&secrets, "cred-ops", SecretAction::Delete).unwrap();
        assert!(secrets.get("cred-ops").unwrap().is_none());
    }

    #[test]
    fn a_failed_secret_save_is_shown() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);

        let _ = update(
            &mut state,
            Message::SecretStored(Err("keyring locked".to_string())),
        );

        assert!(state.error.as_deref().unwrap().contains("keyring locked"));
    }

    #[test]
    fn a_start_up_notice_is_shown_when_loading_worked() {
        let secrets = Arc::new(MemoryStore::new());
        let notice = Some("no keyring".to_string());

        let state = State::new(
            PathBuf::from("unused"),
            Ok(Store::default()),
            secrets,
            notice,
        );

        assert_eq!(state.error.as_deref(), Some("no keyring"));
    }

    fn settings(state: &mut State, message: settings::Message) {
        let _ = update(state, Message::Settings(message));
    }

    #[test]
    fn the_settings_button_opens_the_screen_and_close_returns() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);

        tree(&mut state, tree::Message::SettingsPressed);
        assert_eq!(state.centre, Centre::Settings);

        settings(&mut state, settings::Message::ClosePressed);
        assert_eq!(state.centre, Centre::Sessions);
    }

    #[test]
    fn a_theme_change_applies_at_once_and_is_saved_beside_the_connections_file() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);
        assert!(!theme(&state).extended_palette().is_dark);

        let _ = update(&mut state, Message::SystemModeChanged(Mode::Dark));
        assert!(theme(&state).extended_palette().is_dark);
        let _ = update(&mut state, Message::SystemModeChanged(Mode::Light));

        let dark = Preferences {
            theme: ThemeChoice::Dark,
            ..Preferences::default()
        };
        let state_with_dark = State::new(
            state.path.clone(),
            Ok(Store::default()),
            Arc::new(MemoryStore::new()),
            None,
        )
        .with_preferences(dark.clone());
        assert!(theme(&state_with_dark).extended_palette().is_dark);

        state.preferences = dark.clone();
        state.save_preferences();

        let saved = preferences::load(&preferences::path_beside(&state.path)).unwrap();
        assert_eq!(saved, dark);
    }

    fn locked_state(directory: &tempfile::TempDir) -> State {
        let vault_file = directory.path().join("secrets.toml");
        state(directory, ONE_CONNECTION).with_vault_prompt(vault_file, "no keyring".to_string())
    }

    #[test]
    fn skipping_the_master_password_keeps_secrets_in_memory_and_says_so() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = locked_state(&directory);

        let _ = update(&mut state, Message::Vault(vault::Message::SkipPressed));

        assert!(state.vault.is_none());
        assert_eq!(state.error, Some(fl!("vault-skipped")));
    }

    #[test]
    fn an_unlocked_store_replaces_the_temporary_one_and_closes_the_prompt() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = locked_state(&directory);
        let unlocked = MemoryStore::new();
        unlocked
            .set("cred-ops", &Secret::new("hunter2".to_string()))
            .unwrap();

        let opened = Message::VaultOpened(Ok(Unlocked(Arc::new(unlocked))));
        let _ = update(&mut state, opened);

        assert!(state.vault.is_none());
        assert!(state.secrets.get("cred-ops").unwrap().is_some());
    }

    #[test]
    fn a_failed_unlock_keeps_the_prompt_open() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = locked_state(&directory);

        let failed = Message::VaultOpened(Err("wrong password".to_string()));
        let _ = update(&mut state, failed);

        assert!(state.vault.is_some());
    }

    #[test]
    fn collapsing_a_sidebar_is_remembered_in_the_settings_file() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory, ONE_CONNECTION);
        let saved =
            |state: &State| preferences::load(&preferences::path_beside(&state.path)).unwrap();

        let _ = update(&mut state, Message::TreeCollapseToggled);
        assert!(state.preferences.tree_collapsed);
        assert!(saved(&state).tree_collapsed);
        assert!(!saved(&state).properties_collapsed);

        let _ = update(&mut state, Message::PropertiesCollapseToggled);
        let _ = update(&mut state, Message::TreeCollapseToggled);
        assert!(!saved(&state).tree_collapsed);
        assert!(saved(&state).properties_collapsed);
    }
}
