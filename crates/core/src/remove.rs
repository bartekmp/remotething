//! Removing a folder together with everything inside it.

use crate::model::Store;
use crate::{Error, Result};

impl Store {
    /// Removes a folder with every folder and connection inside it.
    ///
    /// Refused when something that stays uses one of the removed connections
    /// as its jump host.
    pub fn remove_folder_tree(&mut self, folder_id: &str) -> Result<()> {
        let folders = self.folder_and_descendants(folder_id);
        let connections: Vec<String> = self
            .connections
            .iter()
            .filter(|connection| {
                connection
                    .parent
                    .as_ref()
                    .is_some_and(|parent| folders.contains(parent))
            })
            .map(|connection| connection.id.clone())
            .collect();

        // Everything that stays, with the jump host it names.
        let staying_folders = self
            .folders
            .iter()
            .filter(|folder| !folders.contains(&folder.id))
            .map(|folder| (&folder.id, &folder.settings.jump));
        let staying_connections = self
            .connections
            .iter()
            .filter(|connection| !connections.contains(&connection.id))
            .map(|connection| (&connection.id, &connection.settings.jump));
        for (user, jump) in staying_folders.chain(staying_connections) {
            if let Some(jump) = jump
                && connections.contains(jump)
            {
                return Err(Error::JumpInUse {
                    jump: jump.clone(),
                    user: user.clone(),
                });
            }
        }

        self.folders.retain(|folder| !folders.contains(&folder.id));
        self.connections
            .retain(|connection| !connections.contains(&connection.id));
        Ok(())
    }

    /// How many folders and connections are inside a folder, at any depth.
    pub fn count_inside(&self, folder_id: &str) -> (usize, usize) {
        let folders = self.folder_and_descendants(folder_id);
        let connections = self
            .connections
            .iter()
            .filter(|c| c.parent.as_ref().is_some_and(|p| folders.contains(p)))
            .count();
        (folders.len() - 1, connections)
    }

    /// The folder's id and those of all folders below it.
    fn folder_and_descendants(&self, folder_id: &str) -> Vec<String> {
        let mut found = vec![folder_id.to_string()];
        let mut next = 0;
        // `found` grows while it is walked; each pass adds one level.
        while next < found.len() {
            let parent = found[next].clone();
            for folder in &self.folders {
                let is_child = folder.parent.as_deref() == Some(parent.as_str());
                if is_child && !found.contains(&folder.id) {
                    found.push(folder.id.clone());
                }
            }
            next += 1;
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        toml::from_str(
            r#"
            [[folder]]
            id = "a"
            name = "A"

            [[folder]]
            id = "b"
            name = "B"
            parent = "a"

            [[folder]]
            id = "other"
            name = "Other"

            [[connection]]
            id = "in-a"
            name = "in-a"
            protocol = "ssh"
            host = "a.example.net"
            parent = "a"

            [[connection]]
            id = "in-b"
            name = "in-b"
            protocol = "ssh"
            host = "b.example.net"
            parent = "b"

            [[connection]]
            id = "outside"
            name = "outside"
            protocol = "ssh"
            host = "o.example.net"
            parent = "other"
            "#,
        )
        .unwrap()
    }

    #[test]
    fn the_folder_its_subfolders_and_their_connections_go() {
        let mut store = store();

        store.remove_folder_tree("a").unwrap();

        let folders: Vec<&str> = store.folders.iter().map(|f| f.id.as_str()).collect();
        let connections: Vec<&str> = store.connections.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(folders, ["other"]);
        assert_eq!(connections, ["outside"]);
    }

    #[test]
    fn what_is_inside_a_folder_is_counted_at_every_depth() {
        assert_eq!(store().count_inside("a"), (1, 2));
        assert_eq!(store().count_inside("other"), (0, 1));
    }

    #[test]
    fn a_jump_host_used_from_outside_stops_the_removal() {
        let mut store = store();
        store.connections[2].settings.jump = Some("in-b".to_string());

        let error = store.remove_folder_tree("a").unwrap_err();

        assert!(matches!(error, Error::JumpInUse { .. }));
        assert_eq!(store.connections.len(), 3);
    }

    #[test]
    fn a_jump_host_used_only_from_inside_goes_with_its_users() {
        let mut store = store();
        store.connections[1].settings.jump = Some("in-a".to_string());

        assert!(store.remove_folder_tree("a").is_ok());
    }
}
