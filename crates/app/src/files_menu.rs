//! The context menu of a row in the file pane: what it offers, and what each
//! choice does. It opens under the right-clicked row. Drawing is in `files_menu_view.rs`.

use std::sync::Arc;

use iced::Task;
use iced::widget::operation;
use remotething_files::EntryKind;

use crate::files_pane::{self, Message, State, Status};
use crate::files_preview;
use crate::i18n::fl;

/// The id of the text field for a new name.
pub(crate) const NAME_INPUT: &str = "files-menu-name";

/// The menu of one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Menu {
    /// The full remote path of the row.
    pub path: String,
    pub kind: EntryKind,
    pub mode: Mode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The list of actions.
    Actions,
    /// A new name for the row, as typed so far.
    Renaming(String),
    /// The name of a new folder inside the row, as typed so far.
    NewFolder(String),
    /// The question before deleting.
    ConfirmDelete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Preview,
    Download,
    CopyPath,
    Rename,
    NewFolder,
    Delete,
}

/// The actions offered for a kind of row, in the order shown.
pub(crate) fn actions(kind: EntryKind) -> &'static [Action] {
    match kind {
        EntryKind::Folder => &[
            Action::Download,
            Action::NewFolder,
            Action::CopyPath,
            Action::Rename,
            Action::Delete,
        ],
        EntryKind::File | EntryKind::Link => &[
            Action::Preview,
            Action::Download,
            Action::CopyPath,
            Action::Rename,
            Action::Delete,
        ],
        EntryKind::Other => &[Action::CopyPath],
    }
}

/// A name that stays in one folder: not empty, not `.` or `..`, no separator.
pub(crate) fn is_valid_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\0'])
}

/// The last part of a remote path.
pub(crate) fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Opens the menu under a row, which becomes the selected one.
pub(crate) fn open(state: &mut State, path: String, kind: EntryKind) {
    state.selected = Some((path.clone(), kind));
    state.menu = Some(Menu {
        path,
        kind,
        mode: Mode::Actions,
    });
}

/// A choice in the list of actions.
pub(crate) fn choose(state: &mut State, action: Action) -> Task<Message> {
    let Some(menu) = state.menu.as_mut() else {
        return Task::none();
    };
    match action {
        Action::Rename => {
            menu.mode = Mode::Renaming(name_of(&menu.path).to_string());
            operation::focus(NAME_INPUT).chain(operation::select_all(NAME_INPUT))
        }
        Action::NewFolder => {
            menu.mode = Mode::NewFolder(String::new());
            operation::focus(NAME_INPUT)
        }
        Action::Delete => {
            menu.mode = Mode::ConfirmDelete;
            Task::none()
        }
        Action::CopyPath => {
            let path = menu.path.clone();
            state.menu = None;
            iced::clipboard::write(path)
        }
        Action::Preview => {
            state.menu = None;
            files_preview::open(state)
        }
        Action::Download => {
            state.menu = None;
            files_pane::update(state, Message::DownloadPressed).0
        }
    }
}

/// The user typed in the name field.
pub(crate) fn typed(state: &mut State, text: String) {
    if let Some(Menu {
        mode: Mode::Renaming(name) | Mode::NewFolder(name),
        ..
    }) = state.menu.as_mut()
    {
        *name = text;
    }
}

/// Enter or the confirm button: does the rename, the new folder or the delete.
pub(crate) fn submit(state: &mut State) -> Task<Message> {
    let Some(menu) = state.menu.clone() else {
        return Task::none();
    };
    let parent = remotething_files::parent(&menu.path).unwrap_or_else(|| state.root.clone());
    // What to do and the folder to read again afterwards. A name that is not valid does
    // nothing, and the panel keeps waiting for a better one.
    let operation = match &menu.mode {
        Mode::Renaming(name) if is_valid_name(name) => {
            let target = remotething_files::join(&parent, name.trim());
            if target == menu.path {
                state.menu = None;
                return Task::none();
            }
            Operation::Rename(target, parent)
        }
        Mode::NewFolder(name) if is_valid_name(name) => {
            let target = remotething_files::join(&menu.path, name.trim());
            // Its parent opens, so the new folder shows.
            state.expanded.insert(menu.path.clone());
            Operation::NewFolder(target, menu.path.clone())
        }
        Mode::ConfirmDelete => Operation::Delete(menu.kind == EntryKind::Folder, parent),
        Mode::Renaming(_) | Mode::NewFolder(_) | Mode::Actions => return Task::none(),
    };
    let Some(files) = state.files.clone() else {
        return Task::none();
    };
    state.menu = None;
    let path = menu.path;
    let work = async move {
        match operation {
            Operation::Rename(target, folder) => files
                .rename(&path, &target)
                .await
                .map(|()| folder)
                .map_err(|error| error.to_string()),
            Operation::NewFolder(target, folder) => files
                .create_folder(&target)
                .await
                .map(|()| folder)
                .map_err(|error| error.to_string()),
            Operation::Delete(is_folder, folder) => {
                delete(&files, &path, is_folder).await.map(|()| folder)
            }
        }
    };
    Task::perform(work, Message::OperationDone)
}

/// What a submitted menu does on the server, with the folder to read again afterwards.
enum Operation {
    Rename(String, String),
    NewFolder(String, String),
    Delete(bool, String),
}

/// Deletes a file, or a folder that is empty. A folder with files in it is refused by the server.
async fn delete(
    files: &Arc<dyn remotething_files::RemoteFs>,
    path: &str,
    folder: bool,
) -> Result<(), String> {
    let deleted = if folder {
        files.remove_folder(path).await
    } else {
        files.remove_file(path).await
    };
    deleted.map_err(|error| error.to_string())
}

/// An operation ended: read the folder again, or show why it failed.
pub(crate) fn done(state: &mut State, result: Result<String, String>) -> Task<Message> {
    match result {
        Ok(folder) => {
            state.selected = None;
            state.listings.remove(&folder);
            state.list(&folder)
        }
        Err(reason) => {
            state.status = Some(Status::Failed(reason));
            Task::none()
        }
    }
}

/// The question that goes with deleting.
pub(crate) fn delete_question(path: &str, kind: EntryKind) -> String {
    let name = name_of(path);
    if kind == EntryKind::Folder {
        fl!("files-delete-folder-question", name = name)
    } else {
        fl!("files-delete-question", name = name)
    }
}

#[cfg(test)]
#[path = "files_menu_tests.rs"]
mod tests;
