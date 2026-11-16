//! What the file pane does to put transfers in line: asking the server first,
//! turning folders into their files, and starting what is next.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;

use crate::files_folders;
use crate::files_pane::{Message, State};
use crate::files_transfer;
use crate::transfer_queue::Direction;

/// Asks the server whether the upload would replace a file, before sending anything.
pub(crate) fn check_upload(state: &mut State, local: PathBuf) -> Task<Message> {
    let (Some(files), Some(remote)) = (&state.files, state.upload_target(&local)) else {
        return Task::none();
    };
    let files = Arc::clone(files);
    state.status = None;
    Task::perform(
        async move {
            let exists = files.exists(&remote).await;
            (local, exists.map_err(|error| error.to_string()))
        },
        |(local, exists)| Message::UploadChecked(local, exists),
    )
}

/// Puts an upload of `local`, a file or a folder, in the queue.
pub(crate) fn start_upload(state: &mut State, local: PathBuf) -> Task<Message> {
    let Some(remote) = state.upload_target(&local) else {
        return Task::none();
    };
    state.status = None;
    if local.is_dir() {
        return expand(Direction::Upload, async move {
            let walked =
                tokio::task::spawn_blocking(move || files_folders::uploads(&local, &remote)).await;
            walked
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())
        });
    }
    state.queue.add(Direction::Upload, local, remote);
    start_ready(state)
}

/// Puts a download of the selected file in the queue.
pub(crate) fn start_download(state: &mut State, local: PathBuf) -> Task<Message> {
    let Some(remote) = state.selected_file().map(str::to_string) else {
        return Task::none();
    };
    state.status = None;
    state.queue.add(Direction::Download, local, remote);
    start_ready(state)
}

/// Lists the selected remote folder and queues a download of every file in it,
/// into a folder of the same name inside `destination`.
pub(crate) fn download_folder(state: &mut State, destination: PathBuf) -> Task<Message> {
    let (Some(files), Some(remote)) = (state.files.clone(), state.selected_folder()) else {
        return Task::none();
    };
    let remote = remote.to_string();
    let name = remote.rsplit('/').next().unwrap_or("folder").to_string();
    let local_folder = destination.join(name);
    state.status = None;
    expand(Direction::Download, async move {
        let found = files
            .files_under(&remote)
            .await
            .map_err(|error| error.to_string())?;
        Ok(files_folders::downloads(&remote, &local_folder, &found))
    })
}

/// Waits for the files a folder stands for, then hands them over as `Message::Expanded`.
pub(crate) fn expand(
    direction: Direction,
    work: impl Future<Output = Result<Vec<(PathBuf, String)>, String>> + Send + 'static,
) -> Task<Message> {
    Task::perform(work, move |result| Message::Expanded(direction, result))
}

/// Asks for everything that is showing again; folded folders are forgotten.
pub(crate) fn refresh(state: &mut State) -> Task<Message> {
    let mut showing: Vec<String> = state.expanded.iter().cloned().collect();
    showing.push(state.root.clone());
    state.listings.clear();
    Task::batch(showing.iter().map(|folder| state.list(folder)))
}

/// Starts the transfers that are next in line, as many as there is room for.
pub(crate) fn start_ready(state: &mut State) -> Task<Message> {
    let Some(files) = state.files.clone() else {
        return Task::none();
    };
    let mut tasks = Vec::new();
    for id in state.queue.take_ready() {
        let Some(job) = state.queue.job(id) else {
            continue;
        };
        let (task, handle) = files_transfer::start(Arc::clone(&files), job);
        state.queue.set_handle(id, handle);
        tasks.push(task);
    }
    Task::batch(tasks)
}
