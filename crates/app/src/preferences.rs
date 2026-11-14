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
