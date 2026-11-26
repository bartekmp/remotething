//! Remotething: remote desktops, terminals and file transfers in one tree.
//!
//! Usage: remotething [--config <connections file>] | --version

// On Windows, a window program: no console opens beside it.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod branding;
mod capture_files;
mod challenge;
mod clipboard_actions;
mod console_recording;
mod credential_actions;
mod credentials;
mod desktop;
mod desktop_cursor;
mod desktop_files;
mod desktop_open;
mod desktop_size;
mod desktop_view;
mod display;
mod ellipsis;
mod features;
mod file_clipboard;
mod file_details;
mod file_dialog;
mod file_preview;
mod files_columns;
mod files_dual;
mod files_folders;
mod files_keys;
mod files_menu;
mod files_menu_view;
mod files_pane;
mod files_preview;
mod files_preview_view;
mod files_queueing;
mod files_selection;
mod files_transfer;
mod files_transfers_view;
mod files_view;
mod forwards;
mod ftp_tab;
mod ftp_view;
mod host_key_view;
mod i18n;
mod icons;
mod image_clipboard;
mod import_preview;
mod input;
mod items;
mod jump;
mod keys;
mod keysym;
mod launcher;
mod link;
mod local_shell;
mod menu;
mod meter;
mod motion;
mod note;
mod organise;
mod palette;
mod panel_actions;
mod paste;
mod picture;
mod preferences;
mod properties;
mod putty_import;
mod putty_source;
mod quality_choice;
mod recent;
mod removal;
mod rows;
mod scancodes;
mod screenshot;
mod search;
mod session;
mod session_strip;
mod session_update;
mod session_view;
mod settings;
mod settings_actions;
mod shortcuts;
mod status_bar;
mod tab_bar;
mod tabs;
mod tabs_view;
mod terminal;
mod terminal_look;
mod terminal_mouse;
mod terminal_search;
mod terminal_settings;
mod terminal_theme;
mod terminal_view;
mod theme;
mod theme_schemes;
mod transfer_queue;
mod tree;
mod tree_actions;
mod vault;
mod video_recording;
mod vnc_tab;
mod vnc_view;
mod window;
mod window_actions;
mod wsl_import;
mod wsl_source;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use remotething_core::Store;
use remotething_secrets::{KeyringStore, MemoryStore, SecretStore, TimeoutStore, run_with_timeout};

use crate::i18n::fl;

/// Finding the keyring is quick when there is one.
const KEYRING_OPEN_TIMEOUT: Duration = Duration::from_secs(5);
/// A single call may wait for the user to type the keyring password.
const KEYRING_CALL_TIMEOUT: Duration = Duration::from_secs(60);

/// The main window: the logo as its icon, and the name desktops match with the launcher entry.
/// The window opens as big on screen whatever the interface size, so a bigger interface
/// does not make a window that no longer fits.
fn window_settings(
    interface_scale: u16,
    placement: Option<preferences::WindowPlacement>,
) -> iced::window::Settings {
    let mut settings = iced::window::Settings {
        icon: branding::window_icon(),
        size: scaled_window_size(interface_scale),
        ..iced::window::Settings::default()
    };
    if let Some(placement) = placement.filter(|placement| placement.is_usable()) {
        settings.size = iced::Size::new(placement.width as f32, placement.height as f32);
    }
    #[cfg(target_os = "linux")]
    {
        // Inside a Flatpak the desktop entry is named after the app id, and the window
        // must carry the same name for the desktop to find its icon.
        settings.platform_specific.application_id =
            std::env::var("FLATPAK_ID").unwrap_or_else(|_| "remotething".to_string());
    }
    settings
}

const WINDOW_SIZE: iced::Size = iced::Size::new(1024.0, 768.0);

fn scaled_window_size(interface_scale: u16) -> iced::Size {
    let factor = f32::from(interface_scale.clamp(50, 300)) / 100.0;
    iced::Size::new(WINDOW_SIZE.width / factor, WINDOW_SIZE.height / factor)
}

fn main() -> Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments == ["--version"] {
        // Stays English and unformatted: scripts and bug reports read it.
        println!("remotething {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let path = match config_argument(&arguments)? {
        Some(path) => path,
        None => default_config_path()?,
    };
    let loaded = load_store(&path);

    // The language must be fixed before the first translated string is looked up.
    let (preferences, settings_problem) = load_preferences(&path);
    i18n::set_language_override(preferences.language.clone());

    picture::detect_gpu();
    let keyring = open_keyring();
    let vault_file = vault::path_beside(&path);

    let interface_scale = preferences.interface_scale;
    let placement = preferences.window;

    let boot = move || {
        let state = match &keyring {
            Ok(keyring) => app::State::new(
                path.clone(),
                loaded.clone(),
                Arc::clone(keyring),
                settings_problem.clone(),
            ),
            // No keyring: ask for the master password of the encrypted file.
            // Secrets saved before it is unlocked last until the app quits.
            Err(problem) => app::State::new(
                path.clone(),
                loaded.clone(),
                Arc::new(MemoryStore::new()),
                settings_problem.clone(),
            )
            .with_vault_prompt(vault_file.clone(), problem.clone()),
        };
        let state = state.with_preferences(preferences.clone());
        // Ask the desktop once whether it is light or dark; changes arrive by subscription.
        let ask_mode = iced::system::theme().map(app::Message::SystemModeChanged);
        // The scale factor of the screen the window opens on.
        let ask_scale = iced::window::oldest()
            .and_then(iced::window::scale_factor)
            .map(app::Message::ScaleChanged);
        // Asking at start-up, because on X11 a window cannot be created maximized.
        let maximize = if placement.is_some_and(|placement| placement.maximized) {
            iced::window::oldest().and_then(|id| iced::window::maximize(id, true))
        } else {
            iced::Task::none()
        };
        (state, iced::Task::batch([ask_mode, ask_scale, maximize]))
    };
    iced::application(boot, app::update, window::view)
        .title(app::title)
        .theme(app::theme)
        .scale_factor(app::scale_factor)
        .subscription(app::subscription)
        .window(window_settings(interface_scale, placement))
        .settings(iced::Settings {
            default_text_size: theme::TEXT_BODY.into(),
            default_font: theme::FONT,
            fonts: vec![
                theme::INTER_REGULAR.into(),
                theme::INTER_BOLD.into(),
                icons::FONT_BYTES.into(),
                terminal_view::FONT_REGULAR.into(),
                terminal_view::FONT_BOLD.into(),
            ],
            ..iced::Settings::default()
        })
        .run()
        .context("the window could not be opened")
}

/// The saved preferences, or defaults plus the reason the file could not be read.
fn load_preferences(connections_file: &Path) -> (preferences::Preferences, Option<String>) {
    match preferences::load(&preferences::path_beside(connections_file)) {
        Ok(preferences) => (preferences, None),
        Err(reason) => {
            let problem = format!("{} {reason}", fl!("error-settings-load"));
            (preferences::Preferences::default(), Some(problem))
        }
    }
}

/// The system keyring, or the reason there is none.
fn open_keyring() -> Result<Arc<dyn SecretStore>, String> {
    match run_with_timeout(KEYRING_OPEN_TIMEOUT, KeyringStore::open) {
        Ok(keyring) => {
            let store = TimeoutStore::new(Arc::new(keyring), KEYRING_CALL_TIMEOUT);
            Ok(Arc::new(store))
        }
        Err(error) => Err(error.to_string()),
    }
}

/// The path given with `--config`, if any.
fn config_argument(arguments: &[String]) -> Result<Option<PathBuf>> {
    match arguments {
        [] => Ok(None),
        [flag, path] if flag == "--config" => Ok(Some(PathBuf::from(path))),
        _ => bail!("usage: remotething [--config <connections file>]"),
    }
}

/// `~/.config/remotething/connections.toml` on Linux, `%APPDATA%\remotething\...` on Windows.
fn default_config_path() -> Result<PathBuf> {
    let variable = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    let directory = config_directory(
        variable("APPDATA"),
        variable("XDG_CONFIG_HOME"),
        variable("HOME"),
    )
    .context("cannot find the configuration directory: set HOME or use --config")?;
    Ok(directory.join("remotething").join("connections.toml"))
}

/// Picks the platform's configuration directory from the environment.
fn config_directory(
    appdata: Option<OsString>,
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    if cfg!(windows) {
        return appdata.map(PathBuf::from);
    }
    if let Some(directory) = xdg_config_home {
        return Some(PathBuf::from(directory));
    }
    home.map(|home| PathBuf::from(home).join(".config"))
}

/// A missing file is a fresh start, not an error.
fn load_store(path: &Path) -> Result<Store, String> {
    if !path.exists() {
        return Ok(Store::default());
    }
    Store::load(path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_keeps_its_size_on_screen_at_any_interface_size() {
        assert_eq!(scaled_window_size(100), iced::Size::new(1024.0, 768.0));
        assert_eq!(scaled_window_size(200), iced::Size::new(512.0, 384.0));
        assert_eq!(scaled_window_size(0), iced::Size::new(2048.0, 1536.0));
    }

    fn arguments(list: &[&str]) -> Vec<String> {
        list.iter().map(|argument| argument.to_string()).collect()
    }

    #[test]
    fn no_arguments_means_the_default_path() {
        assert_eq!(config_argument(&arguments(&[])).unwrap(), None);
    }

    #[test]
    fn the_config_flag_gives_the_path() {
        let path = config_argument(&arguments(&["--config", "/tmp/c.toml"])).unwrap();

        assert_eq!(path, Some(PathBuf::from("/tmp/c.toml")));
    }

    #[test]
    fn anything_else_is_a_usage_error() {
        assert!(config_argument(&arguments(&["--help"])).is_err());
        assert!(config_argument(&arguments(&["--config"])).is_err());
    }

    #[cfg(not(windows))]
    #[test]
    fn xdg_config_home_wins_over_home() {
        let directory = config_directory(None, Some("/xdg".into()), Some("/home/me".into()));

        assert_eq!(directory, Some(PathBuf::from("/xdg")));
    }

    #[cfg(not(windows))]
    #[test]
    fn without_xdg_the_directory_is_dot_config_in_home() {
        let directory = config_directory(None, None, Some("/home/me".into()));

        assert_eq!(directory, Some(PathBuf::from("/home/me/.config")));
    }

    #[test]
    fn an_empty_environment_gives_no_directory() {
        assert_eq!(config_directory(None, None, None), None);
    }

    #[test]
    fn a_missing_file_loads_as_an_empty_store() {
        let store = load_store(Path::new("/nonexistent/connections.toml")).unwrap();

        assert_eq!(store, Store::default());
    }

    #[test]
    fn a_broken_file_reports_why() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        std::fs::write(&path, "[[connection]]\nid = ").unwrap();

        let reason = load_store(&path).unwrap_err();

        assert!(reason.contains("cannot parse"));
    }
}
