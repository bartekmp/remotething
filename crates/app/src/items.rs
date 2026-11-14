//! What the window does to folders and connections: add, save, remove, rename, move.

use remotething_core::{Connection, Folder, Protocol, Settings};

use crate::app::State;
use crate::i18n::fl;
use crate::organise;
use crate::tree::{DropTarget, Selection};

impl State {
    /// New items go into the selected folder, or beside the selected connection.
    pub(crate) fn folder_for_new_item(&self) -> Option<String> {
        match self.tree.selected()? {
            Selection::Folder(id) => Some(id.clone()),
            Selection::Connection(id) => self
                .store
                .connections
                .iter()
                .find(|connection| &connection.id == id)
                .and_then(|connection| connection.parent.clone()),
        }
    }

    pub(crate) fn add_folder(&mut self, parent: Option<String>) {
        let name = fl!("tree-new-folder-name");
        let folder = Folder {
            id: self.store.new_folder_id(&name),
            name,
            parent,
            icon: None,
            settings: Settings::default(),
        };
        let selection = Selection::Folder(folder.id.clone());
        if let Some(parent) = &folder.parent {
            self.tree.expand(parent);
        }

        let mut updated = self.store.clone();
        updated.folders.push(folder);
        if self.commit(updated) {
            self.select(Some(selection));
        }
    }

    pub(crate) fn add_connection(&mut self, parent: Option<String>) {
        let name = fl!("tree-new-connection-name");
        let connection = Connection {
            id: self.store.new_connection_id(&name),
            name,
            parent,
            protocol: Protocol::Ssh,
            host: fl!("tree-new-connection-host"),
            icon: None,
            settings: Settings::default(),
        };
        let selection = Selection::Connection(connection.id.clone());
        if let Some(parent) = &connection.parent {
            self.tree.expand(parent);
        }

        let mut updated = self.store.clone();
        updated.connections.push(connection);
        if self.commit(updated) {
            self.select(Some(selection));
        }
    }

    pub(crate) fn save_draft(&mut self) {
        let (Some(draft), Some(selection)) = (&self.draft, self.tree.selected()) else {
            return;
        };
        match draft.apply(&self.store, selection) {
            Ok(updated) => {
                self.commit(updated);
            }
            Err(reason) => self.error = Some(reason),
        }
    }

    pub(crate) fn delete(&mut self, item: &Selection) {
        let mut updated = self.store.clone();
        let removed = match item {
            Selection::Folder(id) => updated.remove_folder(id),
            Selection::Connection(id) => updated.remove_connection(id),
        };
        match removed {
            Ok(()) => {
                if self.commit(updated) {
                    self.select(None);
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// Opens a tab for the connection.
    pub(crate) fn open(&mut self, connection_id: &str) {
        // The tree only offers connections from this store, so this resolves.
        if let Ok(connection) = self.store.resolve(connection_id) {
            self.tabs.open(connection);
        }
    }

    pub(crate) fn rename(&mut self, item: &Selection, name: &str) {
        match organise::renamed(&self.store, item, name) {
            Ok(updated) => self.commit_and_reload(updated),
            Err(reason) => self.error = Some(reason),
        }
    }

    pub(crate) fn set_icon(&mut self, item: &Selection, icon: Option<&str>) {
        let updated = organise::with_icon(&self.store, item, icon);
        self.commit_and_reload(updated);
    }

    /// Carries out a drop in the tree. A folder that receives something is unfolded.
    pub(crate) fn move_item(&mut self, source: &Selection, target: &DropTarget) {
        match organise::moved(&self.store, source, target) {
            Ok(updated) => {
                if let DropTarget::Row(Selection::Folder(id)) = target {
                    self.tree.expand(id);
                }
                self.commit_and_reload(updated);
            }
            Err(reason) => self.error = Some(reason),
        }
    }

    /// Saves, then shows the new values in the properties panel.
    fn commit_and_reload(&mut self, updated: remotething_core::Store) {
        if self.commit(updated) {
            let selection = self.tree.selected().cloned();
            self.select(selection);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced::Point;
    use remotething_core::Store;
    use remotething_secrets::MemoryStore;

    use super::*;
    use crate::app::{Message, update};
    use crate::{menu, tree};

    const TREE: &str = r#"
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
    "#;

    fn state(directory: &tempfile::TempDir) -> State {
        let path = directory.path().join("connections.toml");
        std::fs::write(&path, TREE).unwrap();
        let store = Store::load(&path).unwrap();
        State::new(path, Ok(store), Arc::new(MemoryStore::new()), None)
    }

    fn send(state: &mut State, message: Message) {
        let _ = update(state, message);
    }

    fn right_click(state: &mut State, target: Option<Selection>) {
        send(state, Message::PointerMoved(Point::new(40.0, 90.0)));
        send(state, Message::Tree(tree::Message::MenuRequested(target)));
    }

    fn folder(id: &str) -> Selection {
        Selection::Folder(id.to_string())
    }

    fn connection(id: &str) -> Selection {
        Selection::Connection(id.to_string())
    }

    fn on_disk(state: &State) -> Store {
        Store::load(&state.path).unwrap()
    }

    #[test]
    fn a_right_click_opens_the_menu_at_the_pointer_and_selects_the_row() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);

        right_click(&mut state, Some(connection("web-01")));

        let expected = menu::State::new(Some(connection("web-01")), Point::new(40.0, 90.0));
        assert_eq!(state.menu, Some(expected));
        assert_eq!(state.tree.selected(), Some(&connection("web-01")));
    }

    #[test]
    fn escape_and_a_click_outside_close_the_menu() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);

        right_click(&mut state, None);
        send(&mut state, Message::EscapePressed);
        assert_eq!(state.menu, None);

        right_click(&mut state, None);
        send(&mut state, Message::Menu(menu::Message::Dismissed));
        assert_eq!(state.menu, None);
    }

    #[test]
    fn new_connection_on_a_folder_goes_inside_it_whatever_is_selected() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);
        state.select(Some(folder("prod")));

        right_click(&mut state, Some(folder("lab")));
        send(&mut state, Message::Menu(menu::Message::NewConnection));

        let added = on_disk(&state).connections.pop().unwrap();
        assert_eq!(added.parent.as_deref(), Some("lab"));
        assert_eq!(state.menu, None);
    }

    #[test]
    fn new_folder_on_the_empty_space_goes_to_the_top_level() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);
        state.select(Some(folder("prod")));

        right_click(&mut state, None);
        send(&mut state, Message::Menu(menu::Message::NewFolder));

        assert_eq!(on_disk(&state).folders.pop().unwrap().parent, None);
    }

    #[test]
    fn connect_in_the_menu_opens_a_tab() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);

        right_click(&mut state, Some(connection("web-01")));
        send(&mut state, Message::Menu(menu::Message::Connect));

        assert_eq!(state.tabs.open_ids(), ["web-01"]);
    }

    #[test]
    fn rename_in_the_menu_then_enter_saves_the_new_name() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);

        right_click(&mut state, Some(folder("prod")));
        send(&mut state, Message::Menu(menu::Message::Rename));
        let typed = tree::Message::RenameChanged("Live".to_string());
        send(&mut state, Message::Tree(typed));
        send(&mut state, Message::Tree(tree::Message::RenameSubmitted));

        assert_eq!(on_disk(&state).folders[0].name, "Live");
        // The properties panel shows the new name too.
        assert_eq!(state.draft.as_ref().unwrap().name, "Live");
    }

    #[test]
    fn escape_during_a_rename_keeps_the_old_name() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);

        right_click(&mut state, Some(folder("prod")));
        send(&mut state, Message::Menu(menu::Message::Rename));
        let typed = tree::Message::RenameChanged("Live".to_string());
        send(&mut state, Message::Tree(typed));
        send(&mut state, Message::EscapePressed);
        send(&mut state, Message::Tree(tree::Message::RenameSubmitted));

        assert_eq!(on_disk(&state).folders[0].name, "Production");
    }

    #[test]
    fn change_icon_keeps_the_menu_open_until_an_icon_is_picked() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);

        right_click(&mut state, Some(connection("web-01")));
        send(&mut state, Message::Menu(menu::Message::ChangeIcon));
        assert!(state.menu.is_some());

        let picked = menu::Message::IconPicked(Some("database"));
        send(&mut state, Message::Menu(picked));

        assert_eq!(state.menu, None);
        let saved = on_disk(&state);
        assert_eq!(saved.connections[0].icon.as_deref(), Some("database"));
    }

    #[test]
    fn remove_in_the_menu_deletes_the_item_and_refuses_a_full_folder() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);

        right_click(&mut state, Some(folder("prod")));
        send(&mut state, Message::Menu(menu::Message::Remove));
        assert!(state.error.is_some());
        assert_eq!(on_disk(&state).folders.len(), 2);

        right_click(&mut state, Some(folder("lab")));
        send(&mut state, Message::Menu(menu::Message::Remove));
        assert_eq!(on_disk(&state).folders.len(), 1);
    }

    #[test]
    fn dragging_a_connection_onto_a_folder_moves_it_on_disk() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);
        let pressed = tree::Message::ConnectionSelected("web-01".to_string());

        send(&mut state, Message::Tree(pressed));
        let entered = tree::Message::RowEntered(folder("lab"));
        send(&mut state, Message::Tree(entered));
        send(&mut state, Message::PointerReleased);

        let saved = on_disk(&state);
        assert_eq!(saved.connections[0].parent.as_deref(), Some("lab"));
        assert_eq!(state.draft.as_ref().unwrap().parent.as_deref(), Some("lab"));
    }

    #[test]
    fn a_refused_drop_changes_nothing_and_says_why() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);
        state.move_item(&folder("lab"), &DropTarget::Row(folder("prod")));

        state.move_item(&folder("prod"), &DropTarget::Row(folder("lab")));

        assert!(state.error.is_some());
        assert_eq!(on_disk(&state).folders[0].parent, None);
    }

    #[test]
    fn a_plain_click_release_moves_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state(&directory);
        let before = on_disk(&state);

        let pressed = tree::Message::ConnectionSelected("web-01".to_string());
        send(&mut state, Message::Tree(pressed));
        send(&mut state, Message::PointerReleased);

        assert_eq!(on_disk(&state), before);
    }
}
