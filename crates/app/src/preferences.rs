//! What the user chose in Settings, kept in `settings.toml` beside the connections file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub theme: ThemeChoice,
    /// A language code such as `en`. `None` follows the system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// True when the connection tree is folded to a narrow rail.
    #[serde(default, skip_serializing_if = "is_false")]
    pub tree_collapsed: bool,
    /// What the tab bar does when the tabs do not fit on one line.
    #[serde(default)]
    pub tab_layout: TabLayout,
    /// The optional features, which are all off until switched on.
    #[serde(default, skip_serializing_if = "Features::is_empty")]
    pub features: Features,
}

/// The optional features of the app. Each one that is off leaves no trace in the window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Features {
    /// Saving pictures of remote desktops and the screen of SSH terminals.
    #[serde(default, skip_serializing_if = "is_false")]
    pub screenshots: bool,
    /// Recording of SSH terminals, in the asciicast format.
    #[serde(default, skip_serializing_if = "is_false")]
    pub console_recording: bool,
    /// Whether recordings also keep what was typed. Passwords are typed, so this is off by default.
    #[serde(default, skip_serializing_if = "is_false")]
    pub record_keystrokes: bool,
    /// Whether every SSH session is recorded from its start.
    #[serde(default, skip_serializing_if = "is_false")]
    pub record_every_session: bool,
}

impl Features {
    /// True when nothing is switched on, so the table can be left out of the file.
    pub fn is_empty(&self) -> bool {
        *self == Features::default()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabLayout {
    /// One line, with a button that lists the tabs that do not fit.
    #[default]
    SingleLine,
    /// As many lines as needed, up to three.
    Wrap,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

/// The settings file lives in the same directory as the connections file.
pub fn path_beside(connections_file: &Path) -> PathBuf {
    connections_file.with_file_name("settings.toml")
}

/// Where recordings go: a folder beside the connections file.
pub fn recordings_beside(connections_file: &Path) -> PathBuf {
    connections_file.with_file_name("recordings")
}

/// Where screenshots go: a folder beside the connections file.
pub fn screenshots_beside(connections_file: &Path) -> PathBuf {
    connections_file.with_file_name("screenshots")
}

/// A missing file means defaults. A broken one is an error, so it is not silently overwritten.
pub fn load(path: &Path) -> Result<Preferences, String> {
    if !path.exists() {
        return Ok(Preferences::default());
    }
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    toml::from_str(&text).map_err(|error| error.to_string())
}

pub fn save(preferences: &Preferences, path: &Path) -> Result<(), String> {
    let text = toml::to_string_pretty(preferences).map_err(|error| error.to_string())?;
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_are_off_by_default_and_not_written_while_off() {
        let preferences = Preferences::default();

        assert!(preferences.features.is_empty());
        let text = toml::to_string_pretty(&preferences).unwrap();
        assert!(!text.contains("features"), "{text}");
    }

    #[test]
    fn a_switched_on_feature_is_saved_and_loaded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        let mut preferences = Preferences::default();
        preferences.features.console_recording = true;

        save(&preferences, &path).unwrap();

        assert!(load(&path).unwrap().features.console_recording);
    }

    #[test]
    fn recordings_go_in_a_folder_beside_the_connections_file() {
        let folder = recordings_beside(Path::new("/home/me/.config/remotething/connections.toml"));

        assert_eq!(folder, Path::new("/home/me/.config/remotething/recordings"));
    }

    #[test]
    fn a_missing_file_gives_defaults() {
        let preferences = load(Path::new("/nonexistent/settings.toml")).unwrap();

        assert_eq!(preferences, Preferences::default());
        assert_eq!(preferences.theme, ThemeChoice::System);
        assert_eq!(preferences.language, None);
    }

    #[test]
    fn saved_preferences_load_back_the_same() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new").join("settings.toml");
        let preferences = Preferences {
            theme: ThemeChoice::Dark,
            language: Some("en".to_string()),
            tree_collapsed: true,
            tab_layout: TabLayout::Wrap,
            features: Features {
                screenshots: true,
                console_recording: true,
                record_keystrokes: false,
                record_every_session: true,
            },
        };

        save(&preferences, &path).unwrap();

        assert_eq!(load(&path).unwrap(), preferences);
    }

    #[test]
    fn an_empty_file_gives_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        std::fs::write(&path, "").unwrap();

        assert_eq!(load(&path).unwrap(), Preferences::default());
    }

    #[test]
    fn a_broken_file_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        std::fs::write(&path, "theme = \"purple\"").unwrap();

        assert!(load(&path).is_err());
    }

    #[test]
    fn the_settings_file_sits_beside_the_connections_file() {
        let path = path_beside(Path::new("/config/remotething/connections.toml"));

        assert_eq!(path, PathBuf::from("/config/remotething/settings.toml"));
    }
}
