//! Checks that every reference in the tree points at something real.

use std::collections::HashSet;

use crate::model::Store;
use crate::{Error, Result};

impl Store {
    /// Reports the first broken reference, duplicate id or folder cycle.
    ///
    /// A store that passes can be resolved for every connection without error.
    pub fn validate(&self) -> Result<()> {
        check_unique(
            self.credentials
                .iter()
                .map(|credential| credential.id.as_str()),
        )?;
        check_unique(self.folders.iter().map(|folder| folder.id.as_str()))?;
        check_unique(
            self.connections
                .iter()
                .map(|connection| connection.id.as_str()),
        )?;

        for folder in &self.folders {
            // Walking up also catches unknown parents and cycles.
            self.ancestors(&folder.id, folder.parent.as_deref())?;
            self.check_jump(&folder.id, folder.settings.jump.as_deref())?;
            self.check_credential(&folder.id, folder.settings.credential.as_deref())?;
        }

        for connection in &self.connections {
            self.ancestors(&connection.id, connection.parent.as_deref())?;
            self.check_jump(&connection.id, connection.settings.jump.as_deref())?;
            self.check_credential(&connection.id, connection.settings.credential.as_deref())?;
        }

        Ok(())
    }

    fn check_credential(&self, item: &str, credential: Option<&str>) -> Result<()> {
        let Some(credential) = credential else {
            return Ok(());
        };
        let exists = self.credentials.iter().any(|known| known.id == credential);
        if exists {
            return Ok(());
        }
        Err(Error::UnknownCredential {
            item: item.to_string(),
            credential: credential.to_string(),
        })
    }

    fn check_jump(&self, item: &str, jump: Option<&str>) -> Result<()> {
        let Some(jump) = jump else {
            return Ok(());
        };
        let exists = self
            .connections
            .iter()
            .any(|connection| connection.id == jump);
        if exists {
            return Ok(());
        }
        Err(Error::UnknownJump {
            item: item.to_string(),
            jump: jump.to_string(),
        })
    }
}

fn check_unique<'a>(ids: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(Error::DuplicateId(id.to_string()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Store {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn a_consistent_tree_passes() {
        let store = parse(
            r#"
            [[folder]]
            id = "prod"
            name = "Production"

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
        );

        store.validate().unwrap();
    }

    #[test]
    fn duplicate_connection_ids_are_rejected() {
        let store = parse(
            r#"
            [[connection]]
            id = "web"
            name = "first"
            protocol = "ssh"
            host = "a"

            [[connection]]
            id = "web"
            name = "second"
            protocol = "ssh"
            host = "b"
            "#,
        );

        assert!(matches!(store.validate(), Err(Error::DuplicateId(id)) if id == "web"));
    }

    #[test]
    fn a_missing_parent_folder_is_rejected() {
        let store = parse(
            r#"
            [[connection]]
            id = "web"
            parent = "nowhere"
            name = "web"
            protocol = "ssh"
            host = "a"
            "#,
        );

        let error = store.validate().unwrap_err();

        assert!(matches!(error, Error::UnknownParent { parent, .. } if parent == "nowhere"));
    }

    #[test]
    fn a_missing_jump_host_is_rejected() {
        let store = parse(
            r#"
            [[connection]]
            id = "web"
            name = "web"
            protocol = "ssh"
            host = "a"
            jump = "nobody"
            "#,
        );

        let error = store.validate().unwrap_err();

        assert!(matches!(error, Error::UnknownJump { jump, .. } if jump == "nobody"));
    }

    #[test]
    fn a_missing_credential_is_rejected() {
        let store = parse(
            r#"
            [[connection]]
            id = "web"
            name = "web"
            protocol = "ssh"
            host = "a"
            credential = "cred-gone"
            "#,
        );

        let error = store.validate().unwrap_err();

        assert!(
            matches!(error, Error::UnknownCredential { credential, .. } if credential == "cred-gone")
        );
    }

    #[test]
    fn duplicate_credential_ids_are_rejected() {
        let store = parse(
            r#"
            [[credential]]
            id = "cred-ops"
            name = "first"
            kind = "password"

            [[credential]]
            id = "cred-ops"
            name = "second"
            kind = "ask"
            "#,
        );

        assert!(matches!(store.validate(), Err(Error::DuplicateId(id)) if id == "cred-ops"));
    }

    #[test]
    fn a_folder_inside_itself_is_rejected() {
        let store = parse(
            r#"
            [[folder]]
            id = "a"
            name = "a"
            parent = "b"

            [[folder]]
            id = "b"
            name = "b"
            parent = "a"
            "#,
        );

        assert!(matches!(store.validate(), Err(Error::FolderCycle(_))));
    }
}
