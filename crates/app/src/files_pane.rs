//! The file pane of a session: the remote folders as a tree, with upload and
//! download. Drawing is in `files_view.rs`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use remotething_files::{Entry, EntryKind, Progress, SftpFs, Transferred};
use remotething_remote::ssh::Connection;

use crate::file_dialog;
use crate::files_transfer;
use crate::i18n::fl;
use crate::transfer_queue::{Direction, Queue};

pub const WIDTH: f32 = 320.0;

/// What is known about the contents of one folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Listing {
    Loading,
    Loaded(Vec<Entry>),
    Failed(String),
}

/// One visible line of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub depth: usize,
    /// The full remote path.
    pub path: String,
    pub entry: Entry,
}

pub struct State {
    /// `None` until the server's file service has started.
    pub(crate) files: Option<Arc<SftpFs>>,
    /// The folder shown at the top of the tree.
    pub(crate) root: String,
    pub(crate) listings: HashMap<String, Listing>,
    pub(crate) expanded: HashSet<String>,
    /// The highlighted row, as path and kind.
    pub(crate) selected: Option<(String, EntryKind)>,
    /// The uploads and downloads of this pane, waiting, running and over.
    pub(crate) queue: Queue,
    /// An upload waiting for a yes, because its remote file exists: local file and remote path.
    pub(crate) pending_replace: Option<(PathBuf, String)>,
    /// The last problem or result, shown at the bottom.
    pub(crate) status: Option<Status>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Status {
    Failed(String),
}

/// The started file service on its way to the window.
#[derive(Clone)]
pub struct Ready {
    files: Arc<SftpFs>,
    home: String,
}

impl std::fmt::Debug for Ready {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Ready({})", self.home)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Opened(Result<Ready, String>),
    Listed(String, Result<Vec<Entry>, String>),
    FolderToggled(String),
    Selected(String, EntryKind),
    UpPressed,
    RefreshPressed,
    UploadPressed,
    /// The local file to upload, or `None` when the dialog was cancelled.
    UploadPicked(Option<PathBuf>),
    /// Whether the remote file an upload would write already exists.
    UploadChecked(PathBuf, Result<bool, String>),
    /// "Replace" in the question about overwriting.
    ReplaceConfirmed,
    /// "Cancel" in the question about overwriting.
    ReplaceCancelled,
    DownloadPressed,
    /// Where to save the download, or `None` when the dialog was cancelled.
    DownloadPicked(Option<PathBuf>),
    /// How far a running transfer has come.
    TransferProgress(u64, Progress),
    /// A transfer ended, with its result.
    TransferEnded(u64, Result<Transferred, String>),
    /// "Cancel" on a transfer.
    CancelPressed(u64),
    /// "Retry" on a failed or cancelled transfer.
    RetryPressed(u64),
    /// "Clear" on the list of transfers.
    ClearPressed,
    ClosePressed,
}

/// What the pane asks its session to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Close,
}

impl State {
    /// Starts the file service on the session's connection.
    pub fn open(connection: Connection) -> (Self, Task<Message>) {
        let state = Self::empty();
        let task = Task::perform(
            async move {
                let files = SftpFs::open(&connection)
                    .await
                    .map_err(|error| error.to_string())?;
                let home = files.home().await.map_err(|error| error.to_string())?;
                Ok(Ready {
                    files: Arc::new(files),
                    home,
                })
            },
            Message::Opened,
        );
        (state, task)
    }

    pub(crate) fn empty() -> Self {
        Self {
            files: None,
            root: String::new(),
            listings: HashMap::new(),
            expanded: HashSet::new(),
            selected: None,
            queue: Queue::default(),
            pending_replace: None,
            status: None,
        }
    }

    /// Asks the server for a folder's contents, unless that is already under way.
    pub(crate) fn list(&mut self, folder: &str) -> Task<Message> {
        let Some(files) = &self.files else {
            return Task::none();
        };
        if self.listings.get(folder) == Some(&Listing::Loading) {
            return Task::none();
        }
        self.listings.insert(folder.to_string(), Listing::Loading);
        let files = Arc::clone(files);
        let folder = folder.to_string();
        Task::perform(
            async move {
                let listed = files.list(&folder).await;
                (folder, listed.map_err(|error| error.to_string()))
            },
            |(folder, listed)| Message::Listed(folder, listed),
        )
    }

    /// The tree as the lines to draw: the root's contents, and under each
    /// unfolded folder its own.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        self.push_rows(&self.root, 0, &mut rows);
        rows
    }

    pub(crate) fn push_rows(&self, folder: &str, depth: usize, rows: &mut Vec<Row>) {
        let Some(Listing::Loaded(entries)) = self.listings.get(folder) else {
            return;
        };
        for entry in entries {
            let path = remotething_files::join(folder, &entry.name);
            let is_open = entry.kind == EntryKind::Folder && self.expanded.contains(&path);
            rows.push(Row {
                depth,
                path: path.clone(),
                entry: entry.clone(),
            });
            if is_open {
                self.push_rows(&path, depth + 1, rows);
            }
        }
    }

    /// Where an upload goes: into the selected folder, beside the selected
    /// file, or into the root when nothing is selected.
    pub(crate) fn upload_folder(&self) -> String {
        match &self.selected {
            Some((path, EntryKind::Folder)) => path.clone(),
            Some((path, _)) => remotething_files::parent(path).unwrap_or_else(|| self.root.clone()),
            None => self.root.clone(),
        }
    }

    /// The remote path an upload of `local` would write to.
    pub(crate) fn upload_target(&self, local: &std::path::Path) -> Option<String> {
        let name = local.file_name()?.to_string_lossy();
        Some(remotething_files::join(&self.upload_folder(), &name))
    }

    /// The selected path, when it is something that can be downloaded.
    pub(crate) fn selected_file(&self) -> Option<&str> {
        match &self.selected {
            Some((path, EntryKind::File | EntryKind::Link)) => Some(path),
            _ => None,
        }
    }
}

pub fn update(state: &mut State, message: Message) -> (Task<Message>, Option<Event>) {
    let task = match message {
        Message::Opened(Ok(ready)) => {
            state.files = Some(ready.files);
            state.root = ready.home;
            let root = state.root.clone();
            state.list(&root)
        }
        Message::Opened(Err(reason)) => {
            state.status = Some(Status::Failed(reason));
            Task::none()
        }
        Message::Listed(folder, listed) => {
            let listing = match listed {
                Ok(entries) => Listing::Loaded(entries),
                Err(reason) => Listing::Failed(reason),
            };
            state.listings.insert(folder, listing);
            Task::none()
        }
        Message::FolderToggled(path) => {
            state.selected = Some((path.clone(), EntryKind::Folder));
            if state.expanded.remove(&path) {
                Task::none()
            } else {
                state.expanded.insert(path.clone());
                match state.listings.get(&path) {
                    Some(Listing::Loaded(_)) => Task::none(),
                    _ => state.list(&path),
                }
            }
        }
        Message::Selected(path, kind) => {
            state.selected = Some((path, kind));
            Task::none()
        }
        Message::UpPressed => match remotething_files::parent(&state.root) {
            Some(parent) => {
                state.root = parent.clone();
                state.selected = None;
                state.list(&parent)
            }
            None => Task::none(),
        },
        Message::RefreshPressed => {
            // Everything showing is asked for again; folded folders are forgotten.
            let mut showing: Vec<String> = state.expanded.iter().cloned().collect();
            showing.push(state.root.clone());
            state.listings.clear();
            Task::batch(showing.iter().map(|folder| state.list(folder)))
        }
        Message::UploadPressed => Task::perform(
            file_dialog::open_any(fl!("files-upload")),
            Message::UploadPicked,
        ),
        Message::UploadPicked(Some(local)) => check_upload(state, local),
        Message::UploadChecked(local, Ok(false)) => start_upload(state, local),
        Message::UploadChecked(local, Ok(true)) => {
            if let Some(remote) = state.upload_target(&local) {
                state.pending_replace = Some((local, remote));
            }
            Task::none()
        }
        Message::UploadChecked(_, Err(reason)) => {
            state.status = Some(Status::Failed(reason));
            Task::none()
        }
        Message::ReplaceConfirmed => match state.pending_replace.take() {
            Some((local, _)) => start_upload(state, local),
            None => Task::none(),
        },
        Message::ReplaceCancelled => {
            state.pending_replace = None;
            Task::none()
        }
        Message::DownloadPressed => match state.selected_file() {
            Some(path) => {
                let name = path.rsplit('/').next().unwrap_or(path).to_string();
                let dialog = file_dialog::save_any(fl!("files-download"), name);
                Task::perform(dialog, Message::DownloadPicked)
            }
            None => Task::none(),
        },
        Message::DownloadPicked(Some(local)) => start_download(state, local),
        // A cancelled dialog.
        Message::UploadPicked(None) | Message::DownloadPicked(None) => Task::none(),
        Message::TransferProgress(id, progress) => {
            state.queue.progress(id, progress);
            Task::none()
        }
        Message::TransferEnded(id, result) => {
            // After an upload the folder it went into shows the new file.
            let uploaded_to = state
                .queue
                .job(id)
                .filter(|job| job.direction == Direction::Upload && result.is_ok())
                .and_then(|job| remotething_files::parent(&job.remote));
            state.queue.ended(id, result);
            let refresh = match uploaded_to {
                Some(folder) => state.list(&folder),
                None => Task::none(),
            };
            Task::batch([refresh, start_ready(state)])
        }
        Message::CancelPressed(id) => {
            state.queue.cancel(id);
            start_ready(state)
        }
        Message::RetryPressed(id) => {
            state.queue.retry(id);
            start_ready(state)
        }
        Message::ClearPressed => {
            state.queue.clear_finished();
            Task::none()
        }
        Message::ClosePressed => return (Task::none(), Some(Event::Close)),
    };
    (task, None)
}

/// Asks the server whether the upload would replace a file, before sending anything.
fn check_upload(state: &mut State, local: PathBuf) -> Task<Message> {
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

/// Puts an upload of `local` in the queue, into the folder the selection points at.
fn start_upload(state: &mut State, local: PathBuf) -> Task<Message> {
    let Some(remote) = state.upload_target(&local) else {
        return Task::none();
    };
    state.status = None;
    state.queue.add(Direction::Upload, local, remote);
    start_ready(state)
}

/// Puts a download of the selected file in the queue.
fn start_download(state: &mut State, local: PathBuf) -> Task<Message> {
    let Some(remote) = state.selected_file().map(str::to_string) else {
        return Task::none();
    };
    state.status = None;
    state.queue.add(Direction::Download, local, remote);
    start_ready(state)
}

/// Starts the transfers that are next in line, as many as there is room for.
fn start_ready(state: &mut State) -> Task<Message> {
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

/// A size the way people read it: `512 B`, `1.5 kB`, `20.0 MB`.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["kB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
#[path = "files_pane_tests.rs"]
mod tests;
