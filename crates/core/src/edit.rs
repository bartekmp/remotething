//! Changes to the tree that need more care than setting a field.

use crate::model::Store;
use crate::{Error, Result};

impl Store {
    /// An unused folder id derived from a name, such as `production` or `production-2`.
    pub fn new_folder_id(&self, name: &str) -> String {
        unique_id(name, |id| self.folders.iter().any(|folder| folder.id == id))
    }

    /// An unused connection id derived from a name.
    pub fn new_connection_id(&self, name: &str) -> String {
        unique_id(name, |id| {
            self.connections
                .iter()
                .any(|connection| connection.id == id)
        })
    }

    /// An unused credential id derived from a name.
    pub fn new_credential_id(&self, name: &str) -> String {
        unique_id(name, |id| {
            self.credentials
                .iter()
                .any(|credential| credential.id == id)
        })
    }

    /// Removes a credential. One that a folder or connection still uses is refused.
    pub fn remove_credential(&mut self, credential_id: &str) -> Result<()> {
        let folder_user = self
            .folders
            .iter()
            .find(|folder| folder.settings.credential.as_deref() == Some(credential_id))
            .map(|folder| folder.id.as_str());
        let connection_user = self
            .connections
            .iter()
            .find(|connection| connection.settings.credential.as_deref() == Some(credential_id))
            .map(|connection| connection.id.as_str());

        if let Some(user) = folder_user.or(connection_user) {
            return Err(Error::CredentialInUse {
                credential: credential_id.to_string(),
                user: user.to_string(),
            });
        }

        self.credentials
            .retain(|credential| credential.id != credential_id);
        Ok(())
    }

    /// Removes an empty folder. A folder with anything inside is refused.
    pub fn remove_folder(&mut self, folder_id: &str) -> Result<()> {
        let has_folders = self
            .folders
            .iter()
            .any(|folder| folder.parent.as_deref() == Some(folder_id));
        let has_connections = self
            .connections
            .iter()
            .any(|connection| connection.parent.as_deref() == Some(folder_id));
        if has_folders || has_connections {
            return Err(Error::FolderNotEmpty(folder_id.to_string()));
        }

        self.folders.retain(|folder| folder.id != folder_id);
        Ok(())
    }

    /// Removes a connection. One that others use as their jump host is refused.
    pub fn remove_connection(&mut self, connection_id: &str) -> Result<()> {
        let folder_user = self
            .folders
            .iter()
            .find(|folder| folder.settings.jump.as_deref() == Some(connection_id))
            .map(|folder| folder.id.as_str());
        let connection_user = self
            .connections
            .iter()
            .find(|connection| connection.settings.jump.as_deref() == Some(connection_id))
            .map(|connection| connection.id.as_str());

        if let Some(user) = folder_user.or(connection_user) {
            return Err(Error::JumpInUse {
                jump: connection_id.to_string(),
                user: user.to_string(),
            });
        }

        self.connections
            .retain(|connection| connection.id != connection_id);
        Ok(())
    }

    /// Moves a folder into `new_parent`, or to the top level for `None`.
    /// Moving it into itself or into one of its own sub-folders is refused.
    pub fn move_folder(&mut self, folder_id: &str, new_parent: Option<&str>) -> Result<()> {
        // Walk up from the new parent; meeting the folder itself would close a loop.
        let mut ancestor = new_parent;
        for _ in 0..=self.folders.len() {
            let Some(id) = ancestor else { break };
            if id == folder_id {
                return Err(Error::FolderCycle(folder_id.to_string()));
            }
            ancestor = self
                .folders
                .iter()
                .find(|folder| folder.id == id)
                .and_then(|folder| folder.parent.as_deref());
        }

        if let Some(folder) = self.folders.iter_mut().find(|f| f.id == folder_id) {
            folder.parent = new_parent.map(str::to_string);
        }
        Ok(())
    }

    /// Moves a connection into `new_parent`, or to the top level for `None`.
    ///
    /// It is placed just before the connection `before`, or last when that is `None`.
    pub fn move_connection(
        &mut self,
        connection_id: &str,
        new_parent: Option<&str>,
        before: Option<&str>,
    ) -> Result<()> {
        let unknown = || Error::UnknownConnection(connection_id.to_string());
        let from = self
            .connections
            .iter()
            .position(|connection| connection.id == connection_id)
            .ok_or_else(unknown)?;
        if before == Some(connection_id) {
            return Ok(());
        }

        let mut connection = self.connections.remove(from);
        connection.parent = new_parent.map(str::to_string);
        let to = before
            .and_then(|before| self.connections.iter().position(|c| c.id == before))
            .unwrap_or(self.connections.len());
        self.connections.insert(to, connection);
        Ok(())
    }
}

fn unique_id(name: &str, is_taken: impl Fn(&str) -> bool) -> String {
    let base = slug(name);
    if !is_taken(&base) {
        return base;
    }
    let mut number = 2;
    loop {
        let candidate = format!("{base}-{number}");
        if !is_taken(&candidate) {
            return candidate;
        }
        number += 1;
    }
}

/// Lowercase letters and digits, with every other run of characters as one dash.
fn slug(name: &str) -> String {
    let mut slug = String::new();
    for character in name.chars() {
        if character.is_alphanumeric() {
            slug.extend(character.to_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        return "item".to_string();
    }
    slug.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        toml::from_str(
            r#"
            [[credential]]
            id = "cred-ops"
            name = "Ops team"
            kind = "password"

            [[credential]]
            id = "cred-spare"
            name = "Spare"
            kind = "ask"

            [[folder]]
            id = "prod"
            name = "Production"
            credential = "cred-ops"

            [[folder]]
            id = "empty"
            name = "Empty"

            [[connection]]
            id = "bastion"
            name = "bastion"
            protocol = "ssh"
            host = "bastion.example.net"

            [[connection]]
            id = "web-01"
            parent = "prod"
            name = "web-01"
            protocol = "ssh"
            host = "web-01.example.net"
            jump = "bastion"
            "#,
        )
        .unwrap()
    }

    #[test]
    fn an_id_is_the_name_in_lowercase_with_dashes() {
        assert_eq!(store().new_connection_id("Web Server #3"), "web-server-3");
    }

    #[test]
    fn a_taken_id_gets_a_number() {
        assert_eq!(store().new_connection_id("Web 01"), "web-01-2");
        assert_eq!(store().new_folder_id("prod"), "prod-2");
    }

    #[test]
    fn folders_and_connections_do_not_share_an_id_space() {
        assert_eq!(store().new_folder_id("bastion"), "bastion");
    }

    #[test]
    fn a_name_without_letters_still_gives_an_id() {
        assert_eq!(store().new_folder_id("!!!"), "item");
    }

    #[test]
    fn an_empty_folder_can_be_removed() {
        let mut store = store();

        store.remove_folder("empty").unwrap();

        assert!(store.folders.iter().all(|folder| folder.id != "empty"));
    }

    #[test]
    fn a_folder_with_content_is_not_removed() {
        let mut store = store();

        let error = store.remove_folder("prod").unwrap_err();

        assert!(matches!(error, Error::FolderNotEmpty(id) if id == "prod"));
        assert_eq!(store.folders.len(), 2);
    }

    #[test]
    fn an_unused_credential_can_be_removed() {
        let mut store = store();

        store.remove_credential("cred-spare").unwrap();

        assert_eq!(store.credentials.len(), 1);
        assert_eq!(store.new_credential_id("Cred Ops"), "cred-ops-2");
    }

    #[test]
    fn a_credential_in_use_is_not_removed() {
        let mut store = store();

        let error = store.remove_credential("cred-ops").unwrap_err();

        assert!(matches!(error, Error::CredentialInUse { user, .. } if user == "prod"));
        assert_eq!(store.credentials.len(), 2);
    }

    #[test]
    fn a_connection_can_be_removed() {
        let mut store = store();

        store.remove_connection("web-01").unwrap();

        assert_eq!(store.connections.len(), 1);
    }

    #[test]
    fn a_jump_host_in_use_is_not_removed() {
        let mut store = store();

        let error = store.remove_connection("bastion").unwrap_err();

        assert!(matches!(error, Error::JumpInUse { user, .. } if user == "web-01"));
        assert_eq!(store.connections.len(), 2);
    }

    fn connection_ids(store: &Store) -> Vec<&str> {
        store.connections.iter().map(|c| c.id.as_str()).collect()
    }

    #[test]
    fn a_folder_moves_into_another_folder_and_back_to_the_top() {
        let mut store = store();

        store.move_folder("empty", Some("prod")).unwrap();
        assert_eq!(store.folders[1].parent.as_deref(), Some("prod"));

        store.move_folder("empty", None).unwrap();
        assert_eq!(store.folders[1].parent, None);
    }

    #[test]
    fn a_folder_does_not_move_into_itself_or_its_own_sub_folder() {
        let mut store = store();
        store.move_folder("empty", Some("prod")).unwrap();

        assert!(matches!(
            store.move_folder("prod", Some("prod")),
            Err(Error::FolderCycle(_))
        ));
        assert!(matches!(
            store.move_folder("prod", Some("empty")),
            Err(Error::FolderCycle(_))
        ));
        assert_eq!(store.folders[0].parent, None);
    }

    #[test]
    fn a_connection_moves_to_the_end_of_another_folder() {
        let mut store = store();

        store
            .move_connection("bastion", Some("prod"), None)
            .unwrap();

        assert_eq!(connection_ids(&store), ["web-01", "bastion"]);
        assert_eq!(store.connections[1].parent.as_deref(), Some("prod"));
    }

    #[test]
    fn a_connection_is_placed_before_another_one() {
        let mut store = store();

        store
            .move_connection("web-01", None, Some("bastion"))
            .unwrap();

        assert_eq!(connection_ids(&store), ["web-01", "bastion"]);
        assert_eq!(store.connections[0].parent, None);
    }

    #[test]
    fn moving_an_unknown_connection_is_an_error() {
        let mut store = store();

        assert!(matches!(
            store.move_connection("nope", None, None),
            Err(Error::UnknownConnection(_))
        ));
    }
}
