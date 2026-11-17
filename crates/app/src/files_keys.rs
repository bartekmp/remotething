//! The keyboard in a file browser: arrows to move, Enter to open, F2 to rename,
//! Delete to delete, Tab to change panes when the local pane is showing.

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use iced::widget::operation;
use iced::widget::scrollable::RelativeOffset;
use iced::{Task, widget};
use remotething_files::EntryKind;

use crate::files_dual;
use crate::files_menu::{self, Action, Mode};
use crate::files_pane::{self, Message, Row, State};

/// The widget id of a pane's list, so the keys can scroll it to the selected row.
pub(crate) fn list_id(state: &State) -> widget::Id {
    widget::Id::new(if state.is_local {
        "files-list-local"
    } else {
        "files-list"
    })
}

/// What a key does in the pane that has the keyboard.
pub(crate) fn on_key(state: &mut State, key: &Key, modifiers: Modifiers) -> Task<Message> {
    if state.other.is_some() && *key == Key::Named(Named::Tab) && !modifiers.control() {
        state.focus_other = !state.focus_other;
        return Task::none();
    }
    if state.focus_other
        && let Some(other) = state.other.as_mut()
    {
        return on_key(other, key, modifiers).map(files_dual::wrap);
    }
    if modifiers.control() || modifiers.alt() {
        return Task::none();
    }
    if state.menu.is_some() {
        return on_key_in_menu(state, key);
    }
    if state.preview.is_some() {
        if *key == Key::Named(Named::Escape) {
            state.preview = None;
        }
        return Task::none();
    }
    let Key::Named(named) = key else {
        return Task::none();
    };
    match named {
        Named::ArrowDown => step(state, 1),
        Named::ArrowUp => step(state, -1),
        Named::Home => jump(state, false),
        Named::End => jump(state, true),
        Named::ArrowRight => open_or_enter(state),
        Named::ArrowLeft => close_or_leave(state),
        Named::Enter => activate(state),
        Named::Backspace => send(state, Message::UpPressed),
        Named::F5 => send(state, Message::RefreshPressed),
        Named::F2 => ask(state, Action::Rename),
        Named::Delete => ask(state, Action::Delete),
        _ => Task::none(),
    }
}

/// Escape closes the menu; Enter confirms a delete. A name field takes its own keys.
fn on_key_in_menu(state: &mut State, key: &Key) -> Task<Message> {
    match key {
        Key::Named(Named::Escape) => send(state, Message::MenuDismissed),
        Key::Named(Named::Enter) if is_confirming_delete(state) => {
            send(state, Message::MenuSubmitted)
        }
        _ => Task::none(),
    }
}

fn is_confirming_delete(state: &State) -> bool {
    state
        .menu
        .as_ref()
        .is_some_and(|menu| matches!(menu.mode, Mode::ConfirmDelete))
}

fn send(state: &mut State, message: Message) -> Task<Message> {
    files_pane::update(state, message).0
}

/// The index of the selected row among `rows`.
fn selected_index(state: &State, rows: &[Row]) -> Option<usize> {
    let (selected, _) = state.selected.as_ref()?;
    rows.iter().position(|row| &row.path == selected)
}

/// Selects the row at `index` and scrolls the list to it.
fn select(state: &mut State, rows: &[Row], index: usize) -> Task<Message> {
    let Some(row) = rows.get(index) else {
        return Task::none();
    };
    let selected = Message::Selected(row.path.clone(), row.entry.kind);
    let selecting = send(state, selected);
    let last = rows.len().saturating_sub(1).max(1);
    let offset = RelativeOffset {
        x: 0.0,
        y: index as f32 / last as f32,
    };
    Task::batch([selecting, operation::snap_to(list_id(state), offset)])
}

fn step(state: &mut State, delta: isize) -> Task<Message> {
    let rows = state.rows();
    if rows.is_empty() {
        return Task::none();
    }
    let index = match selected_index(state, &rows) {
        Some(current) => current.saturating_add_signed(delta).min(rows.len() - 1),
        // Nothing selected yet: down starts at the top, up at the bottom.
        None if delta > 0 => 0,
        None => rows.len() - 1,
    };
    select(state, &rows, index)
}

fn jump(state: &mut State, to_end: bool) -> Task<Message> {
    let rows = state.rows();
    let index = if to_end {
        rows.len().saturating_sub(1)
    } else {
        0
    };
    select(state, &rows, index)
}

/// Right: unfolds a closed folder, and on an open one moves into it.
fn open_or_enter(state: &mut State) -> Task<Message> {
    let Some((path, EntryKind::Folder)) = state.selected.clone() else {
        return Task::none();
    };
    if state.expanded.contains(&path) {
        return step(state, 1);
    }
    send(state, Message::FolderToggled(path))
}

/// Left: folds an open folder, and otherwise moves to the folder that holds the row.
fn close_or_leave(state: &mut State) -> Task<Message> {
    let Some((path, kind)) = state.selected.clone() else {
        return Task::none();
    };
    if kind == EntryKind::Folder && state.expanded.contains(&path) {
        return send(state, Message::FolderToggled(path));
    }
    let rows = state.rows();
    let Some(index) = selected_index(state, &rows) else {
        return Task::none();
    };
    let depth = rows[index].depth;
    match rows[..index].iter().rposition(|row| row.depth < depth) {
        Some(parent) => select(state, &rows, parent),
        None => Task::none(),
    }
}

/// Enter: folds or unfolds a folder, and previews a file.
fn activate(state: &mut State) -> Task<Message> {
    match state.selected.clone() {
        Some((path, EntryKind::Folder)) => send(state, Message::FolderToggled(path)),
        Some((_, EntryKind::File | EntryKind::Link)) => send(state, Message::PreviewPressed),
        _ => Task::none(),
    }
}

/// F2 and Delete: the menu's rename or delete step for the selected row.
fn ask(state: &mut State, action: Action) -> Task<Message> {
    let Some((path, kind)) = state.selected.clone() else {
        return Task::none();
    };
    files_menu::open(state, path, kind);
    files_menu::choose(state, action)
}

#[cfg(test)]
#[path = "files_keys_tests.rs"]
mod tests;
