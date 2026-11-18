//! What the window does when the tree or its context menu asks for something.

use iced::Task;
use iced::widget::operation;
use remotething_core::Direction;

use crate::app::{Centre, Message, State};
use crate::menu;
use crate::organise;
use crate::rows;
use crate::tree::{self, Selection};

pub(crate) fn on_tree_event(state: &mut State, event: tree::Event) -> Task<Message> {
    match event {
        tree::Event::SelectionChanged => {
            let selection = state.tree.selected().cloned();
            state.select(selection);
        }
        tree::Event::Open(connection_id) => return state.open(&connection_id),
        tree::Event::Search(query) => {
            let first = rows::matching_rows(&state.store, &query)
                .into_iter()
                .find(|row| matches!(row.kind, rows::RowKind::Connection(_)));
            // With nothing to open the field stays, for another try.
            let Some(first) = first else {
                return Task::none();
            };
            state.tree.escape();
            return state.open(&first.id);
        }
        tree::Event::NewFolder => state.add_folder(state.folder_for_new_item()),
        tree::Event::NewConnection => state.add_connection(state.folder_for_new_item()),
        tree::Event::ShowCredentials => state.centre = Centre::Credentials,
        tree::Event::Menu(target) => {
            if target.is_some() {
                state.select(target.clone());
            }
            state.menu = Some(menu::State::new(target, state.pointer));
        }
        tree::Event::Renamed(item, name) => state.rename(&item, &name),
        tree::Event::Moved { source, target } => state.move_item(&source, &target),
    }
    Task::none()
}

/// Carries out an entry of the context menu on what was right-clicked.
pub(crate) fn on_menu(state: &mut State, message: menu::Message) -> Task<Message> {
    if message == menu::Message::ChangeIcon {
        if let Some(menu) = &mut state.menu {
            menu.show_icons();
        }
        return Task::none();
    }
    let Some(menu) = state.menu.take() else {
        return Task::none();
    };
    let target = menu.target();
    // New items go inside a right-clicked folder, or to the top level.
    let folder = match target {
        Some(Selection::Folder(id)) => Some(id.clone()),
        _ => None,
    };

    match (message, target) {
        (menu::Message::NewFolder, _) => state.add_folder(folder),
        (menu::Message::NewConnection, _) => state.add_connection(folder),
        (menu::Message::Connect, Some(Selection::Connection(id))) => return state.open(id),
        (menu::Message::Edit, Some(_)) => state.properties_open = true,
        (menu::Message::Rename, Some(item)) => {
            if let Some(name) = organise::name_of(&state.store, item) {
                state.tree.start_rename(item.clone(), name);
                return operation::focus(tree::RENAME_INPUT)
                    .chain(operation::select_all(tree::RENAME_INPUT));
            }
        }
        (menu::Message::Duplicate, Some(Selection::Connection(id))) => {
            state.duplicate_connection(id);
        }
        (menu::Message::IconPicked(icon), Some(item)) => state.set_icon(item, icon),
        (menu::Message::MoveUp, Some(item)) => state.shift(item, Direction::Up),
        (menu::Message::MoveDown, Some(item)) => state.shift(item, Direction::Down),
        (menu::Message::Remove, Some(item)) => state.delete(item),
        _ => {}
    }
    Task::none()
}
