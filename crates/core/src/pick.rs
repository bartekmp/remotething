//! Choosing some connections of a tree, with everything they need to work.

use std::collections::HashSet;

use crate::model::Store;

impl Store {
    /// A tree holding only the connections named in `ids`, plus what they
    /// depend on: the folders above them, the credentials they and those
    /// folders use, and the jump hosts they tunnel through (and theirs).
    ///
    /// The order of the original tree is kept. Unknown ids are ignored.
    pub fn pick_connections(&self, ids: &[String]) -> Store {
        let mut connections: HashSet<&str> = ids
            .iter()
            .map(String::as_str)
            .filter(|id| self.connections.iter().any(|c| c.id == *id))
            .collect();

        // Jump hosts may need jump hosts of their own, so repeat until nothing is added.
        loop {
            let folders = self.folders_above(&connections);
            let wanted_jumps = self
                .connections
                .iter()
                .filter(|c| connections.contains(c.id.as_str()))
                .map(|c| &c.settings.jump)
                .chain(
                    self.folders
                        .iter()
                        .filter(|f| folders.contains(f.id.as_str()))
                        .map(|f| &f.settings.jump),
                )
                .flatten();
            let missing: Vec<&str> = wanted_jumps
                .filter(|jump| !connections.contains(jump.as_str()))
                .filter(|jump| self.connections.iter().any(|c| &c.id == *jump))
                .map(String::as_str)
                .collect();
            if missing.is_empty() {
                break;
            }
            connections.extend(missing);
        }

        let folders = self.folders_above(&connections);
        let picked_connections: Vec<_> = self
            .connections
            .iter()
            .filter(|c| connections.contains(c.id.as_str()))
            .cloned()
            .collect();
        let picked_folders: Vec<_> = self
            .folders
            .iter()
            .filter(|f| folders.contains(f.id.as_str()))
            .cloned()
            .collect();
        let used_credentials: HashSet<&str> = picked_connections
            .iter()
            .filter_map(|c| c.settings.credential.as_deref())
            .chain(
                picked_folders
                    .iter()
                    .filter_map(|f| f.settings.credential.as_deref()),
            )
            .collect();
        let credentials = self
            .credentials
            .iter()
            .filter(|c| used_credentials.contains(c.id.as_str()))
            .cloned()
            .collect();

        Store {
            credentials,
            folders: picked_folders,
            connections: picked_connections,
        }
    }

    /// The ids of every folder that holds one of `connections`, directly or not.
    fn folders_above(&self, connections: &HashSet<&str>) -> HashSet<&str> {
        let mut found = HashSet::new();
        for connection in self
            .connections
            .iter()
            .filter(|c| connections.contains(c.id.as_str()))
        {
            let mut next = connection.parent.as_deref();
            while let Some(id) = next {
                // A folder already seen has had its parents followed.
                if !found.insert(id) {
                    break;
                }
                next = self
                    .folders
                    .iter()
                    .find(|folder| folder.id == id)
                    .and_then(|folder| folder.parent.as_deref());
            }
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
            [[credential]]
            id = "ops"
            name = "Ops"
            kind = "ask"

            [[credential]]
            id = "unused"
            name = "Unused"
            kind = "ask"

            [[folder]]
            id = "prod"
            name = "Prod"
            credential = "ops"

            [[folder]]
            id = "eu"
            name = "EU"
            parent = "prod"

            [[folder]]
            id = "lab"
            name = "Lab"

            [[connection]]
            id = "edge"
            name = "edge"
            protocol = "ssh"
            host = "edge"

            [[connection]]
            id = "web"
            name = "web"
            protocol = "ssh"
            host = "web"
            parent = "eu"
            jump = "bastion"

            [[connection]]
            id = "bastion"
            name = "bastion"
            protocol = "ssh"
            host = "bastion"
            jump = "edge"

            [[connection]]
            id = "toy"
            name = "toy"
            protocol = "ssh"
            host = "toy"
            parent = "lab"
            "#,
        )
        .unwrap()
    }

    fn ids<T>(items: &[T], id: impl Fn(&T) -> &str) -> Vec<&str> {
        items.iter().map(id).collect()
    }

    #[test]
    fn a_pick_brings_its_folders_credentials_and_whole_jump_chain() {
        let picked = store().pick_connections(&["web".to_string()]);

        assert_eq!(
            ids(&picked.connections, |c| &c.id),
            ["edge", "web", "bastion"]
        );
        assert_eq!(ids(&picked.folders, |f| &f.id), ["prod", "eu"]);
        assert_eq!(ids(&picked.credentials, |c| &c.id), ["ops"]);
        assert!(picked.validate().is_ok());
    }

    #[test]
    fn what_was_not_asked_for_stays_out() {
        let picked = store().pick_connections(&["toy".to_string()]);

        assert_eq!(ids(&picked.folders, |f| &f.id), ["lab"]);
        assert!(picked.credentials.is_empty());
    }

    #[test]
    fn picking_nothing_gives_an_empty_tree() {
        let picked = store().pick_connections(&["nope".to_string()]);

        assert_eq!(picked, Store::default());
    }
}
