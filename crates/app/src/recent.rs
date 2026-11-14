//! The connections opened most recently, kept in `state.toml` beside the
//! connections file. Losing this file loses nothing that matters.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many connections are remembered.
const LIMIT: usize = 20;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Recent {
    path: PathBuf,
    /// Connection ids, newest first.
    ids: Vec<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct FileContent {
    #[serde(default)]
    recent: Vec<String>,
}

/// The state file lives in the same directory as the connections file.
pub fn path_beside(connections_file: &Path) -> PathBuf {
    connections_file.with_file_name("state.toml")
}

impl Recent {
    /// Reads the list. A missing or unreadable file is an empty list.
    pub fn load(path: &Path) -> Self {
        let content: FileContent = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            path: path.to_path_buf(),
            ids: content.recent,
        }
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    /// Puts a connection at the front of the list and saves it.
    pub fn touch(&mut self, connection_id: &str) {
        self.ids.retain(|id| id != connection_id);
        self.ids.insert(0, connection_id.to_string());
        self.ids.truncate(LIMIT);

        // A list that cannot be saved is only a lost convenience, so no error is shown.
        let content = FileContent {
            recent: self.ids.clone(),
        };
        if let Ok(text) = toml::to_string(&content) {
            let _ = std::fs::write(&self.path, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_connection_comes_first_and_is_listed_once() {
        let directory = tempfile::tempdir().unwrap();
        let mut recent = Recent::load(&directory.path().join("state.toml"));

        recent.touch("a");
        recent.touch("b");
        recent.touch("a");

        assert_eq!(recent.ids(), ["a", "b"]);
    }

    #[test]
    fn the_list_survives_a_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.toml");
        Recent::load(&path).touch("web-01");

        assert_eq!(Recent::load(&path).ids(), ["web-01"]);
    }

    #[test]
    fn only_the_newest_twenty_are_kept() {
        let directory = tempfile::tempdir().unwrap();
        let mut recent = Recent::load(&directory.path().join("state.toml"));

        for number in 0..25 {
            recent.touch(&format!("c{number}"));
        }

        assert_eq!(recent.ids().len(), LIMIT);
        assert_eq!(recent.ids()[0], "c24");
    }

    #[test]
    fn a_broken_file_is_an_empty_list() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.toml");
        std::fs::write(&path, "recent = [").unwrap();

        assert!(Recent::load(&path).ids().is_empty());
    }
}
