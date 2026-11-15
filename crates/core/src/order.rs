//! Changing the order of items that share a folder.

use crate::model::Store;
use crate::{Error, Result};

/// Which way an item moves among its neighbours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Towards the top of its folder.
    Up,
    /// Towards the bottom of its folder.
    Down,
}

impl Store {
    /// Moves a folder one place among the folders with the same parent.
    /// A folder already at that end stays where it is.
    pub fn shift_folder(&mut self, folder_id: &str, direction: Direction) -> Result<()> {
        let position = self
            .folders
            .iter()
            .position(|folder| folder.id == folder_id)
            .ok_or_else(|| Error::UnknownConnection(folder_id.to_string()))?;
        let parent = self.folders[position].parent.clone();
        let siblings: Vec<usize> = (0..self.folders.len())
            .filter(|&index| self.folders[index].parent == parent)
            .collect();
        swap_with_neighbour(&mut self.folders, &siblings, position, direction);
        Ok(())
    }

    /// Moves a connection one place among the connections with the same parent.
    /// A connection already at that end stays where it is.
    pub fn shift_connection(&mut self, connection_id: &str, direction: Direction) -> Result<()> {
        let position = self
            .connections
            .iter()
            .position(|connection| connection.id == connection_id)
            .ok_or_else(|| Error::UnknownConnection(connection_id.to_string()))?;
        let parent = self.connections[position].parent.clone();
        let siblings: Vec<usize> = (0..self.connections.len())
            .filter(|&index| self.connections[index].parent == parent)
            .collect();
        swap_with_neighbour(&mut self.connections, &siblings, position, direction);
        Ok(())
    }
}

/// Swaps the item at `position` with the previous or next of `siblings`, the
/// positions of all items that share its parent, in order.
fn swap_with_neighbour<T>(
    items: &mut [T],
    siblings: &[usize],
    position: usize,
    direction: Direction,
) {
    let Some(place) = siblings.iter().position(|&index| index == position) else {
        return;
    };
    let neighbour = match direction {
        Direction::Up => place.checked_sub(1),
        Direction::Down => Some(place + 1).filter(|next| *next < siblings.len()),
    };
    if let Some(neighbour) = neighbour {
        items.swap(position, siblings[neighbour]);
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
            id = "inner"
            name = "Inner"
            parent = "b"

            [[folder]]
            id = "b"
            name = "B"

            [[folder]]
            id = "c"
            name = "C"

            [[connection]]
            id = "one"
            name = "one"
            protocol = "ssh"
            host = "1"

            [[connection]]
            id = "nested"
            name = "nested"
            protocol = "ssh"
            host = "n"
            parent = "a"

            [[connection]]
            id = "two"
            name = "two"
            protocol = "ssh"
            host = "2"
            "#,
        )
        .unwrap()
    }

    fn top_level_folders(store: &Store) -> Vec<&str> {
        let top = store
            .folders
            .iter()
            .filter(|folder| folder.parent.is_none());
        top.map(|folder| folder.id.as_str()).collect()
    }

    #[test]
    fn a_folder_swaps_with_its_neighbour_under_the_same_parent() {
        let mut store = store();

        store.shift_folder("b", Direction::Up).unwrap();
        assert_eq!(top_level_folders(&store), ["b", "a", "c"]);

        store.shift_folder("b", Direction::Down).unwrap();
        store.shift_folder("b", Direction::Down).unwrap();
        assert_eq!(top_level_folders(&store), ["a", "c", "b"]);
    }

    #[test]
    fn a_folder_at_the_end_stays_put() {
        let mut store = store();

        store.shift_folder("a", Direction::Up).unwrap();
        store.shift_folder("c", Direction::Down).unwrap();

        assert_eq!(top_level_folders(&store), ["a", "b", "c"]);
    }

    #[test]
    fn folders_of_other_parents_are_not_neighbours() {
        let mut store = store();

        store.shift_folder("inner", Direction::Up).unwrap();
        store.shift_folder("inner", Direction::Down).unwrap();

        assert_eq!(top_level_folders(&store), ["a", "b", "c"]);
    }

    #[test]
    fn a_connection_swaps_only_with_connections_in_its_folder() {
        let mut store = store();

        store.shift_connection("two", Direction::Up).unwrap();

        let ids: Vec<&str> = store.connections.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["two", "nested", "one"]);
    }

    #[test]
    fn an_unknown_item_is_an_error() {
        assert!(store().shift_folder("nope", Direction::Up).is_err());
        assert!(store().shift_connection("nope", Direction::Up).is_err());
    }
}
