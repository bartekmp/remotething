//! The tree flattened into the lines to draw.

use std::collections::HashSet;

use remotething_core::{Protocol, Store};

use crate::icons::{self, Icon};
use crate::tree::Selection;

/// One visible line of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub depth: usize,
    pub id: String,
    pub name: String,
    pub icon: Icon,
    pub kind: RowKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Folder { expanded: bool },
    Connection(Protocol),
}

impl Row {
    /// The folder or connection this line stands for.
    pub fn selection(&self) -> Selection {
        match self.kind {
            RowKind::Folder { .. } => Selection::Folder(self.id.clone()),
            RowKind::Connection(_) => Selection::Connection(self.id.clone()),
        }
    }
}

/// Flattens the tree into the lines to draw, skipping what collapsed folders hide.
///
/// Inside each folder, sub-folders come first, then connections, both in file order.
pub fn visible_rows(store: &Store, collapsed: &HashSet<String>) -> Vec<Row> {
    let mut rows = Vec::new();
    push_children(store, collapsed, None, 0, &mut rows);
    rows
}

/// The connections whose name or host contains `query`, ignoring case, each under the folders
/// it sits in. Every folder shown is open, whatever was collapsed.
pub fn matching_rows(store: &Store, query: &str) -> Vec<Row> {
    let query = query.trim().to_lowercase();
    let matching: HashSet<&str> = store
        .connections
        .iter()
        .filter(|connection| {
            connection.name.to_lowercase().contains(&query)
                || connection.host.to_lowercase().contains(&query)
        })
        .map(|connection| connection.id.as_str())
        .collect();
    // The folders above a match are shown so the match keeps its place.
    let mut folders: HashSet<&str> = HashSet::new();
    for connection in store
        .connections
        .iter()
        .filter(|connection| matching.contains(connection.id.as_str()))
    {
        let mut parent = connection.parent.as_deref();
        // A validated store has no cycles; the cap guards one built by hand.
        for _ in 0..=store.folders.len() {
            let Some(id) = parent else { break };
            folders.insert(id);
            parent = store
                .folders
                .iter()
                .find(|folder| folder.id == id)
                .and_then(|folder| folder.parent.as_deref());
        }
    }
    let mut rows = Vec::new();
    push_matching(store, &matching, &folders, None, 0, &mut rows);
    rows
}

fn push_matching(
    store: &Store,
    matching: &HashSet<&str>,
    folders: &HashSet<&str>,
    parent: Option<&str>,
    depth: usize,
    rows: &mut Vec<Row>,
) {
    if depth > store.folders.len() {
        return;
    }
    for folder in store
        .folders
        .iter()
        .filter(|folder| folder.parent.as_deref() == parent && folders.contains(folder.id.as_str()))
    {
        rows.push(Row {
            depth,
            id: folder.id.clone(),
            name: folder.name.clone(),
            icon: icons::for_folder(folder.icon.as_deref()),
            kind: RowKind::Folder { expanded: true },
        });
        push_matching(store, matching, folders, Some(&folder.id), depth + 1, rows);
    }
    for connection in store.connections.iter().filter(|connection| {
        connection.parent.as_deref() == parent && matching.contains(connection.id.as_str())
    }) {
        rows.push(Row {
            depth,
            id: connection.id.clone(),
            name: connection.name.clone(),
            icon: icons::for_connection(connection.icon.as_deref(), connection.protocol),
            kind: RowKind::Connection(connection.protocol),
        });
    }
}

fn push_children(
    store: &Store,
    collapsed: &HashSet<String>,
    parent: Option<&str>,
    depth: usize,
    rows: &mut Vec<Row>,
) {
    // A validated store has no folder cycles; this cap guards one built by hand.
    if depth > store.folders.len() {
        return;
    }

    for folder in store
        .folders
        .iter()
        .filter(|folder| folder.parent.as_deref() == parent)
    {
        let expanded = !collapsed.contains(&folder.id);
        rows.push(Row {
            depth,
            id: folder.id.clone(),
            name: folder.name.clone(),
            icon: icons::for_folder(folder.icon.as_deref()),
            kind: RowKind::Folder { expanded },
        });
        if expanded {
            push_children(store, collapsed, Some(&folder.id), depth + 1, rows);
        }
    }

    let connections = store
        .connections
        .iter()
        .filter(|connection| connection.parent.as_deref() == parent);
    for connection in connections {
        rows.push(Row {
            depth,
            id: connection.id.clone(),
            name: connection.name.clone(),
            icon: icons::for_connection(connection.icon.as_deref(), connection.protocol),
            kind: RowKind::Connection(connection.protocol),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        toml_store(
            r#"
            [[folder]]
            id = "prod"
            name = "Production"

            [[folder]]
            id = "prod-db"
            name = "Databases"
            parent = "prod"

            [[connection]]
            id = "laptop"
            name = "laptop"
            protocol = "vnc"
            host = "laptop.lan"

            [[connection]]
            id = "web-01"
            name = "web-01"
            parent = "prod"
            protocol = "ssh"
            host = "web-01.example.net"

            [[connection]]
            id = "db-01"
            name = "db-01"
            parent = "prod-db"
            protocol = "rdp"
            host = "db-01.example.net"
            "#,
        )
    }

    fn toml_store(text: &str) -> Store {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        std::fs::write(&path, text).unwrap();
        Store::load(&path).unwrap()
    }

    #[test]
    fn a_search_keeps_only_the_matches_under_their_folders() {
        let rows = matching_rows(&store(), "db");

        assert_eq!(outline(&rows), ["prod", "  prod-db", "    db-01"]);
    }

    #[test]
    fn a_search_ignores_case_and_looks_at_the_host_too() {
        assert_eq!(
            outline(&matching_rows(&store(), "WEB-01")),
            ["prod", "  web-01"]
        );
        assert_eq!(outline(&matching_rows(&store(), "laptop.lan")), ["laptop"]);
    }

    #[test]
    fn a_search_with_no_match_shows_nothing() {
        assert!(matching_rows(&store(), "zzz").is_empty());
    }

    #[test]
    fn a_search_opens_folders_that_were_collapsed() {
        // matching_rows takes no collapsed set: what matches is always visible.
        let rows = matching_rows(&store(), "web");

        assert!(matches!(rows[0].kind, RowKind::Folder { expanded: true }));
    }

    fn outline(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| format!("{}{}", "  ".repeat(row.depth), row.id))
            .collect()
    }

    #[test]
    fn rows_nest_folders_before_connections() {
        let rows = visible_rows(&store(), &HashSet::new());

        assert_eq!(
            outline(&rows),
            ["prod", "  prod-db", "    db-01", "  web-01", "laptop"]
        );
    }

    #[test]
    fn a_collapsed_folder_hides_everything_inside_it() {
        let collapsed = HashSet::from(["prod".to_string()]);

        let rows = visible_rows(&store(), &collapsed);

        assert_eq!(outline(&rows), ["prod", "laptop"]);
        assert_eq!(rows[0].kind, RowKind::Folder { expanded: false });
    }

    #[test]
    fn rows_carry_the_chosen_icon_or_a_default() {
        let mut store = store();
        store.folders[0].icon = Some("star".to_string());

        let rows = visible_rows(&store, &HashSet::new());

        assert_eq!(rows[0].icon.name, "star");
        // db-01 is an RDP connection without an icon of its own.
        assert_eq!(rows[2].icon.name, "monitor");
    }

    #[test]
    fn an_empty_store_has_no_rows() {
        assert!(visible_rows(&Store::default(), &HashSet::new()).is_empty());
    }
}
