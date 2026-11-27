//! The window: tree on the left, tabs in the centre, properties on the right.
//! The credentials and settings screens take the centre while they are open.

use std::path::PathBuf;
use std::sync::Arc;

use iced::theme::Mode;
use iced::{Point, Size, Subscription, Task, Theme, keyboard};
use remotething_core::Store;
use remotething_secrets::{KnownHosts, SecretStore};

use crate::clipboard_actions;
use crate::credentials::{self};
use crate::display;
use crate::features;
use crate::i18n::fl;
use crate::import_preview;
use crate::input;
use crate::menu;
use crate::panel_actions;
use crate::preferences::{self, Preferences, ThemeChoice};
use crate::properties::{self, Draft};
use crate::putty_import;
use crate::recent::{self, Recent};
use crate::removal;
use crate::session;
use crate::settings;
use crate::settings_actions;
use crate::tabs::{self, Page};
use crate::terminal_view;
use crate::theme as look;
use crate::tree::{self, Selection};
use crate::tree_actions;
use crate::vault;
use crate::video_recording;
use crate::window_actions;
use crate::wsl_import;

/// The name offered when exporting.
pub(crate) const EXPORT_FILE_NAME: &str = "remotething-connections.json";
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
    /// What the About page says about newer releases.
    pub(crate) update_check: crate::update_check::Status,
    /// The imported terminal themes and the preview on the Terminal page.
    pub(crate) terminal_themes: crate::terminal_settings::State,
    /// A folder with content that waits for the user to confirm its removal.
    pub(crate) removal: Option<removal::Question>,
    /// The item that was just made, until it is closed or changed.
    pub(crate) fresh: Option<crate::items::Fresh>,
    /// Where the pointer is, in window coordinates.
    pub(crate) pointer: Point,
    /// Whether the edge of the connection tree is being dragged.
    pub(crate) tree_resizing: bool,
    pub(crate) window: Size,
    /// Whether the size of the window is shown, while it is being resized.
    pub(crate) size_shown: bool,
    /// Counts the resizes, so only the timer of the latest one hides the size.
    pub(crate) resize_count: u64,
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
    /// GitHub answered the check for updates, or it failed.
    UpdateChecked(Result<String, String>),
    /// The folder chosen for a feature, or `None` when the dialog was cancelled.
    FolderPicked(settings::FolderKind, Option<std::path::PathBuf>),
    Vault(vault::Message),
    /// Creating or unlocking the passwords file finished.
    VaultOpened(Result<Unlocked, String>),
    /// The desktop switched between light and dark.
    SystemModeChanged(Mode),
    /// The collapse or expand button of the connection tree.
    TreeCollapseToggled,
    /// The edge of the connection tree was pressed: the pointer now sets its width.
    TreeResizeStarted,
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
            update_check: crate::update_check::Status::default(),
            terminal_themes,
            removal: None,
            fresh: None,
            pointer: Point::ORIGIN,
            tree_resizing: false,
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
        let columns = self.preferences.file_columns;
        crate::files_columns::Widths::set_saved(crate::files_columns::Widths {
            permissions: f32::from(columns.permissions),
            modified: f32::from(columns.modified),
            size: f32::from(columns.size),
        });
        let unfolded = if self.preferences.tree_collapsed {
            0.0
        } else {
            1.0
        };
        self.tree_slide = crate::motion::Tween::at(unfolded);
        self
    }

    /// Saves the file pane's column widths once a drag has ended.
    fn save_dragged_columns(&mut self) {
        if !crate::files_columns::Widths::take_changed() {
            return;
        }
        let saved = crate::files_columns::Widths::saved();
        self.preferences.file_columns = preferences::FileColumns {
            permissions: saved.permissions.round() as u16,
            modified: saved.modified.round() as u16,
            size: saved.size.round() as u16,
        };
        self.save_preferences();
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
    pub(crate) fn desktop_has_keys(&self) -> bool {
        self.centre() == Centre::Sessions
            && self.vault.is_none()
            && self.menu.is_none()
            && self.removal.is_none()
            && !self.properties_open
            && !self.tree.is_busy()
            && !self.tabs.has_question()
            && self.tabs.front_is_desktop()
            && !self.tabs.front_creates_keys()
    }
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    let task = handle(state, message);
    state.sync_motion();
    state.save_dragged_columns();
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
            if std::mem::take(&mut state.tree_resizing) {
                state.save_preferences();
            }
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
        Message::PointerMoved(position) => {
            state.pointer = position;
            if state.tree_resizing {
                // The tree starts one gap in from the window edge.
                let width = (position.x - look::GAP).round().max(0.0) as u16;
                let range = preferences::TREE_WIDTH_RANGE;
                state.preferences.tree_width = width.clamp(*range.start(), *range.end());
            }
        }
        Message::TreeResizeStarted => state.tree_resizing = true,
        Message::RecordTick => state.tabs.record_tick(),
        Message::ClipboardTick => return clipboard_actions::on_tick(),
        Message::ClipboardFilesRead(files) => {
            return clipboard_actions::on_files_read(state, files);
        }
        Message::ClipboardRead(text) => return clipboard_actions::on_text_read(state, text),
        Message::ClipboardPictureRead(picture) => {
            clipboard_actions::on_picture_read(state, picture)
        }
        Message::WindowResized(size) => return window_actions::on_resized(state, size),
        Message::ResizeSettled(count) => return window_actions::on_resize_settled(state, count),
        Message::WindowMaximized(maximized) => state.remember_window(maximized),
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
            return input::on_key_pressed(
                state,
                &key,
                physical,
                modifiers,
                text.as_deref(),
                captured,
            );
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
        Message::Palette(message) => return panel_actions::on_palette(state, message),
        Message::Properties(message) => return panel_actions::on_properties(state, message),
        Message::Credentials(message) => return panel_actions::on_credentials(state, message),
        Message::Vault(message) => return panel_actions::on_vault(state, message),
        Message::SystemModeChanged(mode) => state.system_mode = mode,
        Message::TreeCollapseToggled => {
            state.preferences.tree_collapsed = !state.preferences.tree_collapsed;
            state.save_preferences();
        }
        Message::SettingsPressed => state.open_settings(Centre::Settings),
        Message::PropertiesClosed => state.close_properties(),
        Message::VaultOpened(result) => panel_actions::on_vault_opened(state, result),
        Message::UpdateChecked(answer) => {
            state.update_check = crate::update_check::finished(answer)
        }
        Message::Settings(message) => {
            let event = settings::update(&mut state.preferences, message);
            return settings_actions::on_settings_event(state, event);
        }
        // `None` is a cancelled dialog.
        Message::ExportFilePicked(None) | Message::ImportFilePicked(None) => {}
        Message::ExportFilePicked(Some(file)) => state.export_to(&file),
        Message::ImportFilePicked(Some(file)) => state.preview_import(&file),
        Message::Preview(message) => settings_actions::on_preview(state, message),
        Message::Putty(message) => return settings_actions::on_putty(state, message),
        Message::Wsl(message) => return settings_actions::on_wsl(state, message),
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

/// How much bigger than normal the window is drawn.
pub fn scale_factor(state: &State) -> f32 {
    f32::from(state.preferences.interface_scale.clamp(50, 300)) / 100.0
}

/// The chosen theme, or the one matching the desktop for "follow the system".
pub fn theme(state: &State) -> Theme {
    let high_contrast = state.preferences.high_contrast;
    let choice = state.preferences.theme;
    let scheme = match choice {
        ThemeChoice::FollowTerminal => terminal_scheme(state),
        ThemeChoice::System | ThemeChoice::Light | ThemeChoice::Dark => None,
    };
    match scheme {
        // High contrast wins over the terminal's colours; it keeps only light or dark.
        Some(scheme) if high_contrast => look::for_mode(look::mode_of(&scheme), true),
        Some(scheme) => look::for_scheme(&scheme),
        None => {
            let mode = match choice {
                ThemeChoice::Light => Mode::Light,
                ThemeChoice::Dark => Mode::Dark,
                _ => state.system_mode,
            };
            look::for_mode(mode, high_contrast)
        }
    }
}

/// The colours of the chosen terminal theme, while that feature is on. Without a terminal
/// theme of its own there is nothing to follow.
fn terminal_scheme(state: &State) -> Option<look::Scheme> {
    if !state.preferences.features.follow_terminal_theme {
        return None;
    }
    let palette = state.terminal_themes.applied_palette(&state.preferences)?;
    Some(look::scheme_of(&palette))
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
