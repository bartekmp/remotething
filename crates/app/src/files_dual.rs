//! Two panes side by side: the remote files and this computer's, with a copy
//! between them. The local pane is a file pane of its own on the local disk;
//! the transfers all run in the queue of the remote pane.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use remotething_files::{EntryKind, LocalFs, RemoteFs};

use crate::files_pane::{Message, State, Status};
use crate::files_queueing::{check_upload, download_folder, start_download};

/// Shows the local pane, or hides it when it is showing.
pub(crate) fn toggle(state: &mut State) -> Task<Message> {
    if state.other.take().is_some() {
        return Task::none();
    }
    let files: Arc<dyn RemoteFs> = Arc::new(LocalFs);
    let (mut local, task) = State::browse(files);
    local.closable = false;
    local.is_local = true;
    state.other = Some(Box::new(local));
    task.map(wrap)
}

/// Passes a message of the local pane on to the pane that holds it.
pub(crate) fn wrap(message: Message) -> Message {
    Message::Other(Box::new(message))
}

/// Copies what is selected on the remote side into the folder showing on the local side.
pub(crate) fn copy_to_local(state: &mut State) -> Task<Message> {
    let Some(other) = &state.other else {
        return Task::none();
    };
    let folder = other.upload_folder();
    if let Some(path) = state.selected_file() {
        let name = path.rsplit('/').next().unwrap_or(path);
        let local = PathBuf::from(remotething_files::join(&folder, name));
        // A copy never replaces a local file silently.
        return match local.exists() {
            true => {
                state.status = Some(Status::Failed(crate::files_dual::exists_message(&local)));
                Task::none()
            }
            false => start_download(state, local),
        };
    }
    if state.selected_folder().is_some() {
        return download_folder(state, PathBuf::from(folder));
    }
    Task::none()
}

/// Copies what is selected on the local side into the folder showing on the remote side.
pub(crate) fn copy_to_remote(state: &mut State) -> Task<Message> {
    let Some(other) = &state.other else {
        return Task::none();
    };
    match &other.selected {
        Some((path, EntryKind::File | EntryKind::Folder)) => {
            let local = PathBuf::from(path);
            check_upload(state, local)
        }
        _ => Task::none(),
    }
}

/// Whether there is something on each side that a copy could take.
pub(crate) fn can_copy_to_local(state: &State) -> bool {
    state.other.is_some() && state.can_download()
}

pub(crate) fn can_copy_to_remote(state: &State) -> bool {
    state.other.as_ref().is_some_and(|other| {
        matches!(
            other.selected,
            Some((_, EntryKind::File | EntryKind::Folder))
        )
    })
}

pub(crate) fn exists_message(local: &std::path::Path) -> String {
    crate::i18n::fl!("files-copy-exists", file = local.display().to_string())
}

#[cfg(test)]
#[path = "files_dual_tests.rs"]
mod tests;
