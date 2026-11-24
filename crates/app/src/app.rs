//! The window: tree on the left, tabs in the centre, properties on the right.
//! The credentials and settings screens take the centre while they are open.

use std::path::PathBuf;
use std::sync::Arc;

use iced::theme::Mode;
use iced::{Point, Size, Subscription, Task, Theme, keyboard};
use remotething_core::Store;
use remotething_secrets::{KnownHosts, SecretStore};

use crate::credentials::{self};
use crate::display;
use crate::features;
use crate::file_clipboard;
use crate::file_dialog;
use crate::i18n::fl;
use crate::image_clipboard;
use crate::import_preview;
use crate::input;
use crate::menu;
use crate::preferences::{self, Preferences, ThemeChoice};
use crate::properties::{self, Draft};
use crate::putty_import;
use crate::recent::{self, Recent};
use crate::removal;
use crate::session;
use crate::settings;
use crate::tabs::{self, Page};
use crate::terminal_view;
use crate::theme as look;
use crate::tree::{self, Selection};
use crate::tree_actions;
use crate::vault;
use crate::video_recording;
use crate::wsl_import;

/// The name offered when exporting.
const EXPORT_FILE_NAME: &str = "remotething-connections.json";
/// The window size iced starts with, until the first size event arrives.
const INITIAL_WINDOW: Size = Size::new(1024.0, 768.0);

pub struct State {
    /// Where the connections file is read from and saved to.
    pub(crate) path: PathBuf,
    pub(crate) store: Store,
    /// The last load, save or edit problem, shown at the top of the window.
    pub(crate) error: Option<String>,
    /// Good news shown in the same place, such as a finished import.
    pub(crate) notice: Option<String>,
    pub(crate) tree: tree::State,
    pub(crate) tabs: tabs::State,
    /// The properties being edited for the selected item.
    pub(crate) draft: Option<Draft>,
    /// Whether the properties panel floats over the centre. It opens for a
    /// new item or on "Edit", and closes on save.
    pub(crate) properties_open: bool,
    /// The command palette, while it is open.
    pub(crate) palette: Option<crate::palette::State>,
    /// Whether the list of shortcuts is showing.
    pub(crate) shortcuts_open: bool,
    /// How far the connection tree is unfolded: 0 is the narrow rail, 1 the whole panel.
    pub(crate) tree_slide: crate::motion::Tween,
    /// How far the properties panel has slid in: 0 is out of sight, 1 in place.
    pub(crate) properties_slide: crate::motion::Tween,
    /// The time of the latest frame, which animations are drawn at.
    pub(crate) now: std::time::Instant,
    pub(crate) credentials: credentials::State,
    pub(crate) putty: putty_import::State,
    pub(crate) wsl: wsl_import::State,
    /// The preview of a Remotething import, while it is shown.
    pub(crate) import: Option<import_preview::State>,
    /// Which screen the settings page shows: the settings themselves or one of the previews.
    pub(crate) settings_screen: Centre,
    pub(crate) secrets: Arc<dyn SecretStore>,
    /// The connections opened most recently, for the page of a new tab.
    pub(crate) recent: Recent,
    /// The SSH host keys accepted so far.
    pub(crate) known_hosts: KnownHosts,
    pub(crate) preferences: Preferences,
    /// The master-password prompt, while the encrypted passwords file is locked.
    pub(crate) vault: Option<vault::State>,
    /// Whether the desktop is light or dark, for the "follow the system" theme.
    pub(crate) system_mode: Mode,
    /// The context menu of the tree, while it is open.
    pub(crate) menu: Option<menu::State>,
    /// The page of the settings screen that is showing.
    pub(crate) settings_page: settings::Tab,
    /// The imported terminal themes and the preview on the Terminal page.
    pub(crate) terminal_themes: crate::terminal_settings::State,
    /// A folder with content that waits for the user to confirm its removal.
    pub(crate) removal: Option<removal::Question>,
    /// The item that was just made, until it is closed or changed.
    pub(crate) fresh: Option<crate::items::Fresh>,
    /// Where the pointer is, in window coordinates.
    pub(crate) pointer: Point,
    pub(crate) window: Size,
    /// Whether the size of the window is shown, while it is being resized.
    pub(crate) size_shown: bool,
    /// Counts the resizes, so only the timer of the latest one hides the size.
    resize_count: u64,
}

/// An unlocked secret store on its way from the worker thread to the window.
#[derive(Clone)]
pub struct Unlocked(pub(crate) Arc<dyn SecretStore>);

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
    /// The preview of a PuTTY import.
    Putty,
    /// The preview of an import of the running WSL distributions.
    Wsl,
    /// The preview of an import of a Remotething file.
    Import,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tree(tree::Message),
    Tabs(tabs::Message),
    Properties(properties::Message),
    Palette(crate::palette::Message),
    Shortcuts(crate::shortcuts::Message),
    Credentials(credentials::Message),
    /// The secret store finished a save or delete.
    SecretStored(Result<(), String>),
    Putty(putty_import::Message),
    Wsl(wsl_import::Message),
    Preview(import_preview::Message),
    Settings(settings::Message),
    /// The folder chosen for a feature, or `None` when the dialog was cancelled.
    FolderPicked(settings::FolderKind, Option<std::path::PathBuf>),
    Vault(vault::Message),
    /// Creating or unlocking the passwords file finished.
    VaultOpened(Result<Unlocked, String>),
    /// The desktop switched between light and dark.
    SystemModeChanged(Mode),
    /// The collapse or expand button of the connection tree.
    TreeCollapseToggled,
    /// The settings button at the bottom of the connection tree.
    SettingsPressed,
    /// The close button of the properties panel.
    PropertiesClosed,
    Menu(menu::Message),
    /// A file was dropped on the window.
    FileDropped(std::path::PathBuf),
    Removal(removal::Message),
    PointerMoved(Point),
    /// The left mouse button went up, anywhere in the window.
    PointerReleased,
    /// A frame is being drawn, while something is moving.
    Frame(std::time::Instant),
    KeyPressed {
        key: keyboard::Key,
        /// The key by its place on the keyboard, for remote desktops.
        physical: keyboard::key::Physical,
        modifiers: keyboard::Modifiers,
        text: Option<String>,
        /// Whether a text field already used the key.
        captured: bool,
    },
    KeyReleased {
        key: keyboard::Key,
        physical: keyboard::key::Physical,
    },
    /// Time for the next picture of the recordings of remote desktops.
    RecordTick,
    /// Time to look at the local clipboard, because a remote desktop is in front.
    ClipboardTick,
    /// The local clipboard text, if it holds any.
    /// The files on the local clipboard, if there are any.
    ClipboardFilesRead(Option<Vec<std::path::PathBuf>>),
    ClipboardRead(Option<String>),
    /// The local clipboard picture, if it holds one and no text.
    ClipboardPictureRead(Option<remotething_rdp::Picture>),
    WindowResized(Size),
    /// Whether the window fills the screen, asked once it stopped changing size.
    WindowMaximized(bool),
    /// The window has not changed size for a moment: the number of the resize it follows.
    ResizeSettled(u64),
    /// The window's scale factor, when it opens or moves to another screen.
    ScaleChanged(f32),
    /// The file chosen for an export, or `None` when the dialog was cancelled.
    ExportFilePicked(Option<PathBuf>),
    /// The file chosen for an import, or `None` when the dialog was cancelled.
    ImportFilePicked(Option<PathBuf>),
}

impl State {
    /// `notice` is a start-up message to show, such as a missing keyring.
    pub fn new(
        path: PathBuf,
        loaded: Result<Store, String>,
        secrets: Arc<dyn SecretStore>,
        notice: Option<String>,
    ) -> Self {
        let (store, mut error) = match loaded {
            Ok(store) => (store, notice),
            Err(reason) => (
                Store::default(),
                Some(format!("{} {reason}", fl!("error-load"))),
            ),
        };
        let recent = Recent::load(&recent::path_beside(&path));
        // A broken file is reported and left alone; nothing can be trusted until it is fixed.
        let known_hosts = match KnownHosts::load(&known_hosts_path(&path)) {
            Ok(known_hosts) => known_hosts,
            Err(reason) => {
                error.get_or_insert(format!("{} {reason}", fl!("error-known-hosts")));
                KnownHosts::default()
            }
        };
        let (terminal_themes, themes_problem) = crate::terminal_settings::State::load(&path);
        if let Some(reason) = themes_problem {
            error.get_or_insert(format!("{} {reason}", fl!("error-terminal-themes")));
        }
        Self {
            path,
            store,
            error,
            notice: None,
            tree: tree::State::default(),
            tabs: tabs::State::default(),
            draft: None,
            properties_open: false,
            palette: None,
            shortcuts_open: false,
            tree_slide: crate::motion::Tween::at(1.0),
            properties_slide: crate::motion::Tween::at(0.0),
            now: std::time::Instant::now(),
            credentials: credentials::State::default(),
            putty: putty_import::State::default(),
            wsl: wsl_import::State::default(),
            import: None,
            settings_screen: Centre::Settings,
            secrets,
            known_hosts,
            recent,
            preferences: Preferences::default(),
            vault: None,
            system_mode: Mode::None,
            menu: None,
            settings_page: settings::Tab::default(),
            terminal_themes,
            removal: None,
            fresh: None,
            pointer: Point::ORIGIN,
            window: INITIAL_WINDOW,
            size_shown: false,
            resize_count: 0,
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
        features::apply(&self.preferences, &self.path);
        self.apply_terminal_look();
        display::set_interface_percent(self.preferences.interface_scale);
        let unfolded = if self.preferences.tree_collapsed {
            0.0
        } else {
            1.0
        };
        self.tree_slide = crate::motion::Tween::at(unfolded);
        self
    }

    /// Sends the sliding panels where the state says they belong. Called after every message,
    /// so whatever changed `tree_collapsed` or `properties_open` is animated the same way.
    fn sync_motion(&mut self) {
        let now = std::time::Instant::now();
        let animate = self.preferences.animations;
        let unfolded = if self.preferences.tree_collapsed {
            0.0
        } else {
            1.0
        };
        if self.tree_slide.target() != unfolded {
            self.tree_slide.retarget(unfolded, now, animate);
        }
        let shown = if self.properties_open { 1.0 } else { 0.0 };
        if self.properties_slide.target() != shown {
            self.properties_slide.retarget(shown, now, animate);
        }
    }

    /// Whether something is moving, so frames are wanted.
    fn is_animating(&self) -> bool {
        self.tree_slide.is_running(self.now)
            || self.properties_slide.is_running(self.now)
            || self.tabs.is_glowing(self.now)
    }

    /// Keeps the size of the window for the next start. A maximized window keeps the
    /// size it had before, so un-maximizing after a restart goes back to it.
    pub(crate) fn remember_window(&mut self, maximized: bool) {
        let previous = self.preferences.window;
        let size = match previous {
            Some(placement) if maximized => (placement.width, placement.height),
            _ => (
                self.window.width.round() as u32,
                self.window.height.round() as u32,
            ),
        };
        let placement = preferences::WindowPlacement {
            width: size.0,
            height: size.1,
            maximized,
        };
        if previous != Some(placement) {
            self.preferences.window = Some(placement);
            self.save_preferences();
        }
    }

    pub(crate) fn save_preferences(&mut self) {
        features::apply(&self.preferences, &self.path);
        let path = preferences::path_beside(&self.path);
        match preferences::save(&self.preferences, &path) {
            Ok(()) => self.error = None,
            Err(reason) => {
                self.error = Some(format!("{} {reason}", fl!("error-settings-save")));
            }
        }
    }

    /// Saves a changed store; the one in memory is replaced only if that works.
    pub(crate) fn commit(&mut self, updated: Store) -> bool {
        let saved = save(&updated, &self.path);
        match saved {
            Ok(()) => {
                self.store = updated;
                self.notice = None;
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

/// `known_hosts.toml`, in the same directory as the connections file.
fn known_hosts_path(connections_file: &std::path::Path) -> PathBuf {
    connections_file.with_file_name("known_hosts.toml")
}

/// Validates and writes the store, creating the directory on first use.
fn save(store: &Store, path: &std::path::Path) -> Result<(), String> {
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    }
    store.save(path).map_err(|error| error.to_string())
}

impl State {
    /// What fills the middle of the window: the tab in front, which may be a page.
    pub(crate) fn centre(&self) -> Centre {
        match self.tabs.front_page() {
            None => Centre::Sessions,
            Some(Page::Credentials) => Centre::Credentials,
            Some(Page::Settings) => self.settings_screen,
        }
    }

    /// Shows the settings in a tab of their own, on the screen asked for.
    pub(crate) fn open_settings(&mut self, screen: Centre) {
        self.settings_screen = screen;
        self.tabs.open_page(Page::Settings);
    }

    /// Whether keys go to a remote desktop: one is in front, and nothing else wants them.
    fn desktop_has_keys(&self) -> bool {
        self.centre() == Centre::Sessions
            && self.vault.is_none()
            && self.menu.is_none()
            && self.removal.is_none()
            && !self.properties_open
            && !self.tree.is_busy()
            && !self.tabs.has_question()
            && self.tabs.front_is_desktop()
    }
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    let task = handle(state, message);
    state.sync_motion();
    task
}

fn handle(state: &mut State, message: Message) -> Task<Message> {
    // The folded sidebar shows only icons; pressing one opens it for what follows.
    if state.preferences.tree_collapsed
        && matches!(
            message,
            Message::SettingsPressed
                | Message::Tree(
                    tree::Message::NewFolderPressed
                        | tree::Message::NewConnectionPressed
                        | tree::Message::CredentialsPressed
                )
        )
    {
        state.preferences.tree_collapsed = false;
        state.save_preferences();
    }
    match message {
        Message::Frame(now) => state.now = now,
        Message::Tree(message) => {
            if let Some(event) = tree::update(&mut state.tree, message) {
                return tree_actions::on_tree_event(state, event);
            }
        }
        Message::PointerReleased => {
            state.tabs.release(state.preferences.animations);
            if let Some(event) = state.tree.release() {
                return tree_actions::on_tree_event(state, event);
            }
        }
        Message::Menu(message) => return tree_actions::on_menu(state, message),
        Message::FolderPicked(kind, Some(folder)) => {
            settings::set_folder(&mut state.preferences, kind, Some(folder));
            state.save_preferences();
        }
        Message::FolderPicked(_, None) => {}
        Message::Removal(message) => return state.answer_removal(message),
        Message::PointerMoved(position) => state.pointer = position,
        Message::RecordTick => state.tabs.record_tick(),
        Message::ClipboardTick => {
            // Files first: copying them in a file manager often puts their paths there as text too.
            return Task::perform(
                async {
                    tokio::task::spawn_blocking(file_clipboard::read)
                        .await
                        .ok()
                        .flatten()
                },
                Message::ClipboardFilesRead,
            );
        }
        Message::ClipboardFilesRead(files) => {
            let offered = files.is_some_and(|files| state.tabs.desktop_clipboard_files(files));
            if !offered {
                return iced::clipboard::read().map(Message::ClipboardRead);
            }
        }
        Message::ClipboardRead(text) => match text.filter(|text| !text.is_empty()) {
            Some(text) => state.tabs.desktop_clipboard(text),
            // No text: there may be a picture.
            None => {
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(image_clipboard::read)
                            .await
                            .ok()
                            .flatten()
                    },
                    Message::ClipboardPictureRead,
                );
            }
        },
        Message::ClipboardPictureRead(picture) => {
            if let Some(picture) = picture {
                state.tabs.desktop_clipboard_picture(picture);
            }
        }
        Message::WindowResized(size) => {
            if size == state.window {
                return Task::none();
            }
            state.window = size;
            state.size_shown = true;
            state.tabs.size_badge = true;
            state.resize_count += 1;
            let count = state.resize_count;
            return Task::perform(
                async { tokio::time::sleep(SIZE_SHOWN_FOR).await },
                move |()| Message::ResizeSettled(count),
            );
        }
        Message::ResizeSettled(count) => {
            if count == state.resize_count {
                state.size_shown = false;
                state.tabs.size_badge = false;
                return iced::window::oldest()
                    .and_then(iced::window::is_maximized)
                    .map(Message::WindowMaximized);
            }
        }
        Message::WindowMaximized(maximized) => {
            state.remember_window(maximized);
        }
        Message::FileDropped(path) => return input::on_file_dropped(state, path),
        Message::ScaleChanged(scale) => {
            display::set_scale(scale);
            state.tabs.rescale_desktops();
        }
        Message::KeyReleased { key, physical } => {
            if state.desktop_has_keys() {
                state.tabs.desktop_key(&key, physical, None, false);
            }
        }
        Message::KeyPressed {
            key,
            physical,
            modifiers,
            text,
            captured,
        } => {
            if state.shortcuts_open {
                input::on_shortcuts_key(state, &key);
                return Task::none();
            }
            if state.palette.is_some() {
                return input::on_palette_key(state, &key, modifiers);
            }
            // A remote desktop in front gets every key except the app's own tab shortcuts.
            let app_shortcut =
                input::Shortcut::of(&key, modifiers).is_some_and(input::Shortcut::beats_a_desktop);
            if !captured && !app_shortcut && state.desktop_has_keys() {
                state
                    .tabs
                    .desktop_key(&key, physical, text.as_deref(), true);
                return Task::none();
            }
            if let Some(shortcut) = input::Shortcut::of(&key, modifiers) {
                return input::on_shortcut(state, shortcut, captured);
            }
            let is_escape = key == keyboard::Key::Named(keyboard::key::Named::Escape);
            if is_escape && (state.menu.is_some() || state.tree.is_busy()) {
                state.menu = None;
                state.tree.escape();
                return Task::none();
            }
            if is_escape && state.removal.is_some() {
                state.removal = None;
                return Task::none();
            }
            if is_escape && state.properties_open {
                state.close_properties();
                return Task::none();
            }
            // In a form, Tab and Shift+Tab move between the fields.
            let is_tab = key == keyboard::Key::Named(keyboard::key::Named::Tab);
            let in_form = state.properties_open
                || state.vault.is_some()
                || state.centre() != Centre::Sessions
                || state.tabs.front_is_asking();
            if is_tab && in_form && !captured && !modifiers.control() && !modifiers.alt() {
                return if modifiers.shift() {
                    iced::widget::operation::focus_previous()
                } else {
                    iced::widget::operation::focus_next()
                };
            }
            // Everything else belongs to the terminal in front, when one is showing.
            let terminal_shows =
                state.centre() == Centre::Sessions && state.vault.is_none() && state.menu.is_none();
            if captured || !terminal_shows {
                return Task::none();
            }
            return state
                .tabs
                .key(&key, modifiers, text.as_deref())
                .map(Message::Tabs);
        }
        Message::Tabs(tabs::Message::Session(
            _,
            session::Message::Terminal(terminal_view::Event::Zoomed(steps)),
        )) => state.zoom_terminal(steps),
        // A message of a page that lives in a tab is the window's own.
        Message::Tabs(tabs::Message::Page(message)) => return update(state, *message),
        Message::Tabs(message) => {
            let mut context = session::Context {
                store: &state.store,
                secrets: &state.secrets,
                known_hosts: &mut state.known_hosts,
            };
            let task = tabs::update(&mut state.tabs, message, &mut context, &mut state.recent)
                .map(Message::Tabs);
            if let Some((tab, target)) = state.tabs.take_save_request() {
                state.save_typed_connection(tab, &target);
            }
            return task;
        }
        Message::Shortcuts(crate::shortcuts::Message::Dismissed) => state.shortcuts_open = false,
        Message::Palette(message) => {
            let Some(palette) = &mut state.palette else {
                return Task::none();
            };
            let entries =
                crate::palette::entries(&state.store, state.recent.ids(), palette.query());
            match crate::palette::update(palette, message, &entries) {
                Some(crate::palette::Event::Chosen(item)) => {
                    state.palette = None;
                    return input::open_palette_item(state, item);
                }
                Some(crate::palette::Event::Close) => state.palette = None,
                None => {}
            }
        }
        Message::Properties(message) => {
            let Some(draft) = &mut state.draft else {
                return Task::none();
            };
            let selected = state.tree.selected().cloned();
            match (properties::update(draft, message), selected) {
                (Some(properties::Event::Save), _) => return state.save_draft(),
                (Some(properties::Event::Delete), Some(item)) => state.delete(&item),
                _ => {}
            }
        }
        Message::Credentials(message) => {
            match credentials::update(&mut state.credentials, &state.store, message) {
                Some(credentials::Event::New) => state.add_credential(),
                Some(credentials::Event::Save) => return state.save_credential(),
                Some(credentials::Event::Delete) => return state.delete_credential(),
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
        Message::SettingsPressed => state.open_settings(Centre::Settings),
        Message::PropertiesClosed => state.close_properties(),
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
            settings::Event::Changed => {
                state.save_preferences();
                display::set_interface_percent(state.preferences.interface_scale);
                state.tabs.rescale_desktops();
            }
            settings::Event::Terminal(message) => return state.terminal_message(message),
            settings::Event::PageChanged(page) => state.settings_page = page,
            settings::Event::OpenRepository => crate::link::open(crate::link::REPOSITORY),
            settings::Event::PickFolder(kind) => {
                return Task::perform(
                    crate::file_dialog::pick_folder(fl!("settings-folder-pick")),
                    move |folder| Message::FolderPicked(kind, folder),
                );
            }
            settings::Event::Export => {
                let dialog =
                    file_dialog::save(fl!("settings-export"), EXPORT_FILE_NAME.to_string());
                return Task::perform(dialog, Message::ExportFilePicked);
            }
            settings::Event::ImportPutty => {
                let (screen, task) = putty_import::State::open();
                state.putty = screen;
                state.open_settings(Centre::Putty);
                return task.map(Message::Putty);
            }
            settings::Event::ImportWsl => {
                let (screen, task) = wsl_import::State::open();
                state.wsl = screen;
                state.open_settings(Centre::Wsl);
                return task.map(Message::Wsl);
            }
            settings::Event::Import => {
                let dialog = file_dialog::open(fl!("settings-import"));
                return Task::perform(dialog, Message::ImportFilePicked);
            }
        },
        // `None` is a cancelled dialog.
        Message::ExportFilePicked(None) | Message::ImportFilePicked(None) => {}
        Message::ExportFilePicked(Some(file)) => state.export_to(&file),
        Message::ImportFilePicked(Some(file)) => state.preview_import(&file),
        Message::Preview(message) => {
            let Some(screen) = &mut state.import else {
                return Task::none();
            };
            match import_preview::update(screen, message) {
                Some(import_preview::Event::Close) => {
                    state.import = None;
                    state.settings_screen = Centre::Settings;
                }
                Some(import_preview::Event::Apply(tree)) => {
                    state.import = None;
                    state.settings_screen = Centre::Settings;
                    state.import_tree(tree);
                }
                None => {}
            }
        }
        Message::Putty(message) => {
            let (task, event) = putty_import::update(&mut state.putty, message);
            match event {
                Some(putty_import::Event::Close) => state.settings_screen = Centre::Settings,
                Some(putty_import::Event::Apply(plan, names)) => state.apply_putty(&plan, &names),
                None => {}
            }
            return task.map(Message::Putty);
        }
        Message::Wsl(message) => {
            let (task, event) = wsl_import::update(&mut state.wsl, message);
            match event {
                Some(wsl_import::Event::Close) => state.settings_screen = Centre::Settings,
                Some(wsl_import::Event::Apply(plan, names)) => state.apply_wsl(&plan, &names),
                None => {}
            }
            return task.map(Message::Wsl);
        }
        Message::SecretStored(Ok(())) => {}
        Message::SecretStored(Err(reason)) => {
            state.error = Some(format!("{} {reason}", fl!("error-secret")));
        }
    }
    Task::none()
}

pub fn title(_state: &State) -> String {
    fl!("app-title")
}

/// How long the size of the window stays on screen after the last resize.
const SIZE_SHOWN_FOR: std::time::Duration = std::time::Duration::from_millis(900);

/// How much bigger than normal the window is drawn.
pub fn scale_factor(state: &State) -> f32 {
    f32::from(state.preferences.interface_scale.clamp(50, 300)) / 100.0
}

/// The chosen theme, or the one matching the desktop for "follow the system".
pub fn theme(state: &State) -> Theme {
    let mode = match state.preferences.theme {
        ThemeChoice::System => state.system_mode,
        ThemeChoice::Light => Mode::Light,
        ThemeChoice::Dark => Mode::Dark,
    };
    look::for_mode(mode, state.preferences.high_contrast)
}

/// Keeps the theme in step when the desktop switches between light and dark.
pub fn subscription(state: &State) -> Subscription<Message> {
    // The clipboard is only looked at while a remote desktop is in front.
    let clipboard = if state.desktop_has_keys() {
        iced::time::every(CLIPBOARD_EVERY).map(|_| Message::ClipboardTick)
    } else {
        Subscription::none()
    };
    // Recordings keep going whichever tab is in front.
    let recording = if state.tabs.any_recording() {
        iced::time::every(video_recording::TICK).map(|_| Message::RecordTick)
    } else {
        Subscription::none()
    };
    // Frames only come while something is moving.
    let frames = if state.is_animating() {
        iced::window::frames().map(Message::Frame)
    } else {
        Subscription::none()
    };
    Subscription::batch([
        frames,
        iced::system::theme_changes().map(Message::SystemModeChanged),
        iced::event::listen_with(input::window_event),
        clipboard,
        recording,
    ])
}

/// How often the local clipboard is looked at while a remote desktop is in front.
const CLIPBOARD_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
