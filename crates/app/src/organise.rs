//! Edits made straight from the tree: rename, change icon, move.
//! Each returns a changed copy of the store and leaves saving to the window.

use remotething_core::Store;

use crate::i18n::fl;
use crate::tree::{DropTarget, Selection};

/// The name shown for a folder or connection, if it still exists.
pub fn name_of(store: &Store, item: &Selection) -> Option<String> {
    match item {
        Selection::Folder(id) => store
            .folders
            .iter()
            .find(|folder| &folder.id == id)
            .map(|folder| folder.name.clone()),
        Selection::Connection(id) => store
            .connections
            .iter()
            .find(|connection| &connection.id == id)
            .map(|connection| connection.name.clone()),
    }
}

/// The store with `item` given a new name. An empty name is refused.
pub fn renamed(store: &Store, item: &Selection, name: &str) -> Result<Store, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(fl!("properties-error-name"));
    }

    let mut updated = store.clone();
    match item {
        Selection::Folder(id) => {
            if let Some(folder) = updated.folders.iter_mut().find(|f| &f.id == id) {
                folder.name = name.to_string();
            }
        }
        Selection::Connection(id) => {
            if let Some(connection) = updated.connections.iter_mut().find(|c| &c.id == id) {
                connection.name = name.to_string();
            }
        }
    }
    Ok(updated)
}

/// The store with `item` given an icon, or its default one for `None`.
pub fn with_icon(store: &Store, item: &Selection, icon: Option<&str>) -> Store {
    let icon = icon.map(str::to_string);
    let mut updated = store.clone();
    match item {
        Selection::Folder(id) => {
            if let Some(folder) = updated.folders.iter_mut().find(|f| &f.id == id) {
                folder.icon = icon;
            }
        }
        Selection::Connection(id) => {
            if let Some(connection) = updated.connections.iter_mut().find(|c| &c.id == id) {
                connection.icon = icon;
            }
        }
    }
    updated
}

/// The store after `source` was dropped on `target`.
///
/// On a folder it goes inside. On a connection it goes into that connection's
/// folder, and a connection is placed just before it. On empty space it goes
/// to the top level.
pub fn moved(store: &Store, source: &Selection, target: &DropTarget) -> Result<Store, String> {
    let (new_parent, before) = match target {
        DropTarget::TopLevel => (None, None),
        DropTarget::Row(Selection::Folder(id)) => (Some(id.clone()), None),
        DropTarget::Row(Selection::Connection(id)) => {
            let parent = store
                .connections
                .iter()
                .find(|connection| &connection.id == id)
                .and_then(|connection| connection.parent.clone());
            (parent, Some(id.as_str()))
        }
    };

    let mut updated = store.clone();
    let result = match source {
        Selection::Folder(id) => updated.move_folder(id, new_parent.as_deref()),
        Selection::Connection(id) => updated.move_connection(id, new_parent.as_deref(), before),
    };
    result.map_err(|error| error.to_string())?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let text = r#"
            [[folder]]
            id = "prod"
            name = "Production"

            [[folder]]
            id = "lab"
            name = "Lab"

            [[connection]]
            id = "web-01"
            name = "web-01"
            parent = "prod"
            protocol = "ssh"
            host = "web-01.example.net"

            [[connection]]
            id = "web-02"
            name = "web-02"
            parent = "prod"
            protocol = "ssh"
            host = "web-02.example.net"

            [[connection]]
            id = "laptop"
            name = "laptop"
            protocol = "vnc"
            host = "laptop.lan"
        "#;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        std::fs::write(&path, text).unwrap();
        Store::load(&path).unwrap()
    }

    fn folder(id: &str) -> Selection {
        Selection::Folder(id.to_string())
    }

    fn connection(id: &str) -> Selection {
        Selection::Connection(id.to_string())
    }

    fn connection_ids(store: &Store) -> Vec<&str> {
        store.connections.iter().map(|c| c.id.as_str()).collect()
    }

    #[test]
    fn renaming_trims_the_name_and_keeps_the_id() {
        let updated = renamed(&store(), &folder("prod"), "  Live  ").unwrap();

        assert_eq!(updated.folders[0].id, "prod");
        assert_eq!(name_of(&updated, &folder("prod")).as_deref(), Some("Live"));
    }

    #[test]
    fn an_empty_name_is_refused() {
        assert!(renamed(&store(), &connection("laptop"), "   ").is_err());
    }

    #[test]
    fn an_icon_is_set_and_cleared() {
        let with = with_icon(&store(), &connection("laptop"), Some("star"));
        assert_eq!(with.connections[2].icon.as_deref(), Some("star"));

        let without = with_icon(&with, &connection("laptop"), None);
        assert_eq!(without.connections[2].icon, None);
    }

    #[test]
    fn a_connection_dropped_on_a_folder_goes_inside() {
        let target = DropTarget::Row(folder("lab"));

        let updated = moved(&store(), &connection("laptop"), &target).unwrap();

        assert_eq!(updated.connections[2].parent.as_deref(), Some("lab"));
    }

    #[test]
    fn a_connection_dropped_on_a_connection_lands_before_it_in_its_folder() {
        let target = DropTarget::Row(connection("web-01"));

        let updated = moved(&store(), &connection("laptop"), &target).unwrap();

        assert_eq!(connection_ids(&updated), ["laptop", "web-01", "web-02"]);
        assert_eq!(updated.connections[0].parent.as_deref(), Some("prod"));
    }

    #[test]
    fn a_folder_dropped_on_a_connection_joins_that_connections_folder() {
        let target = DropTarget::Row(connection("web-01"));

        let updated = moved(&store(), &folder("lab"), &target).unwrap();

        assert_eq!(updated.folders[1].parent.as_deref(), Some("prod"));
    }

    #[test]
    fn anything_dropped_on_empty_space_goes_to_the_top_level() {
        let updated = moved(&store(), &connection("web-01"), &DropTarget::TopLevel).unwrap();

        assert_eq!(updated.connections.last().unwrap().id, "web-01");
        assert_eq!(updated.connections.last().unwrap().parent, None);
    }

    #[test]
    fn a_folder_dropped_into_itself_is_refused() {
        let target = DropTarget::Row(folder("prod"));

        assert!(moved(&store(), &folder("prod"), &target).is_err());
    }
}
