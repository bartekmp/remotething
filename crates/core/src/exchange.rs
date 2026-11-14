//! Moving a connection tree between two copies of the app as one JSON file.
//!
//! The file holds what the connections file holds: no passwords, no key
//! contents, no trusted host keys.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::model::Store;
use crate::{Error, Result};

/// Marks a file as ours, so another JSON file is refused with a clear message.
const FORMAT: &str = "remotething-connections";
/// Raised when the file changes in a way older versions cannot read.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ExportFile {
    format: String,
    version: u32,
    #[serde(flatten)]
    store: Store,
}

/// What an import added.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    /// Credential descriptions added. Their secrets have to be entered again.
    pub credentials: usize,
    /// Folders added.
    pub folders: usize,
    /// Connections added.
    pub connections: usize,
    /// How many of those got a new id because theirs was already in use.
    pub renamed: usize,
}

impl Store {
    /// The whole tree as the text of an export file.
    pub fn to_export(&self) -> Result<String> {
        let file = ExportFile {
            format: FORMAT.to_string(),
            version: VERSION,
            store: self.clone(),
        };
        serde_json::to_string_pretty(&file).map_err(|error| Error::Export(error.to_string()))
    }

    /// Reads an export file. The tree in it must be consistent on its own.
    pub fn from_export(text: &str) -> Result<Store> {
        let file: ExportFile =
            serde_json::from_str(text).map_err(|error| Error::Import(error.to_string()))?;
        if file.format != FORMAT {
            return Err(Error::Import(
                "this is not a Remotething export".to_string(),
            ));
        }
        if file.version > VERSION {
            let reason = format!(
                "the file is version {}, this app reads up to version {VERSION}",
                file.version
            );
            return Err(Error::Import(reason));
        }
        file.store.validate()?;
        Ok(file.store)
    }

    /// Adds everything from `incoming`. Nothing that exists is changed: an
    /// item whose id is taken comes in under a new id, and what points at it
    /// follows.
    pub fn merge(&mut self, incoming: Store) -> Result<ImportSummary> {
        let mut summary = ImportSummary {
            credentials: incoming.credentials.len(),
            folders: incoming.folders.len(),
            connections: incoming.connections.len(),
            renamed: 0,
        };
        let mut merged = self.clone();
        // Old id to the id the item has after the merge, per kind.
        let mut credential_ids = HashMap::new();
        let mut folder_ids = HashMap::new();
        let mut connection_ids = HashMap::new();

        for mut credential in incoming.credentials {
            if merged.credentials.iter().any(|c| c.id == credential.id) {
                let new_id = merged.new_credential_id(&credential.name);
                credential_ids.insert(credential.id.clone(), new_id.clone());
                credential.id = new_id;
            }
            merged.credentials.push(credential);
        }
        let first_folder = merged.folders.len();
        for mut folder in incoming.folders {
            if merged.folders.iter().any(|f| f.id == folder.id) {
                let new_id = merged.new_folder_id(&folder.name);
                folder_ids.insert(folder.id.clone(), new_id.clone());
                folder.id = new_id;
            }
            merged.folders.push(folder);
        }
        let first_connection = merged.connections.len();
        for mut connection in incoming.connections {
            if merged.connections.iter().any(|c| c.id == connection.id) {
                let new_id = merged.new_connection_id(&connection.name);
                connection_ids.insert(connection.id.clone(), new_id.clone());
                connection.id = new_id;
            }
            merged.connections.push(connection);
        }
        summary.renamed = credential_ids.len() + folder_ids.len() + connection_ids.len();

        // Only the new items are rewired; the existing ones keep pointing where they did.
        let follow = |reference: &mut Option<String>, ids: &HashMap<String, String>| {
            if let Some(new_id) = reference.as_ref().and_then(|id| ids.get(id)) {
                *reference = Some(new_id.clone());
            }
        };
        for folder in &mut merged.folders[first_folder..] {
            follow(&mut folder.parent, &folder_ids);
            follow(&mut folder.settings.credential, &credential_ids);
            follow(&mut folder.settings.jump, &connection_ids);
        }
        for connection in &mut merged.connections[first_connection..] {
            follow(&mut connection.parent, &folder_ids);
            follow(&mut connection.settings.credential, &credential_ids);
            follow(&mut connection.settings.jump, &connection_ids);
        }

        merged.validate()?;
        *self = merged;
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        toml::from_str(
            r#"
            [[credential]]
            id = "ops"
            name = "Ops"
            username = "ops"
            kind = "password"

            [[folder]]
            id = "prod"
            name = "Production"
            credential = "ops"

            [[connection]]
            id = "bastion"
            name = "bastion"
            parent = "prod"
            protocol = "ssh"
            host = "bastion.example.net"

            [[connection]]
            id = "web-01"
            name = "web-01"
            parent = "prod"
            protocol = "ssh"
            host = "web-01.example.net"
            jump = "bastion"
            "#,
        )
        .unwrap()
    }

    #[test]
    fn an_export_reads_back_as_the_same_tree() {
        let text = store().to_export().unwrap();

        assert_eq!(Store::from_export(&text).unwrap(), store());
    }

    #[test]
    fn an_export_says_what_it_is() {
        let text = store().to_export().unwrap();

        assert!(text.contains(r#""format": "remotething-connections""#));
        assert!(text.contains(r#""version": 1"#));
    }

    #[test]
    fn other_json_is_refused() {
        assert!(Store::from_export(r#"{"name": "something else"}"#).is_err());
        assert!(Store::from_export(r#"{"format": "other", "version": 1}"#).is_err());
        assert!(Store::from_export("not json").is_err());
    }

    #[test]
    fn a_file_from_a_newer_version_is_refused() {
        let text = r#"{"format": "remotething-connections", "version": 2}"#;

        assert!(Store::from_export(text).is_err());
    }

    #[test]
    fn a_file_with_a_broken_tree_is_refused() {
        let text = r#"{
            "format": "remotething-connections",
            "version": 1,
            "connection": [
                {"id": "a", "name": "a", "protocol": "ssh", "host": "a", "parent": "missing"}
            ]
        }"#;

        assert!(Store::from_export(text).is_err());
    }

    #[test]
    fn importing_into_an_empty_tree_adds_everything_unchanged() {
        let mut target = Store::default();

        let summary = target.merge(store()).unwrap();

        assert_eq!(target, store());
        assert_eq!(
            summary,
            ImportSummary {
                credentials: 1,
                folders: 1,
                connections: 2,
                renamed: 0,
            }
        );
    }

    #[test]
    fn clashing_ids_come_in_under_new_ids_and_references_follow() {
        let mut target = store();
        target.connections[1].host = "kept.example.net".to_string();

        let summary = target.merge(store()).unwrap();

        assert_eq!(summary.renamed, 4);
        // What was there is untouched.
        assert_eq!(target.connections[1].host, "kept.example.net");
        assert_eq!(
            target.connections[1].settings.jump.as_deref(),
            Some("bastion")
        );

        // The imported copies point at each other, not at the old items.
        let folder = &target.folders[1];
        assert_eq!(folder.id, "production");
        assert_eq!(folder.settings.credential.as_deref(), Some("ops-2"));
        let web = &target.connections[3];
        assert_eq!(web.id, "web-01-2");
        assert_eq!(web.parent.as_deref(), Some("production"));
        assert_eq!(web.settings.jump.as_deref(), Some("bastion-2"));
        assert!(target.validate().is_ok());
    }
}
