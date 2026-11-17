//! The file pane of a session: the remote folders as a tree, with upload and
//! download. Drawing is in `files_view.rs`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use remotething_files::{Entry, EntryKind, Progress, RemoteFs, ScpFs, SftpFs, Transferred};
use remotething_remote::ssh::Connection;

use crate::file_dialog;
use crate::file_preview::Content;
use crate::files_dual;
use crate::files_menu::{self, Action, Menu};
use crate::files_preview::{self, Preview};
use crate::files_queueing::{
    check_upload, download_folder, refresh, start_download, start_ready, start_upload,
};
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
    pub(crate) files: Option<Arc<dyn RemoteFs>>,
    /// The folder shown at the top of the tree.
    pub(crate) root: String,
    pub(crate) listings: HashMap<String, Listing>,
    pub(crate) expanded: HashSet<String>,
    /// The highlighted row, as path and kind.
    pub(crate) selected: Option<(String, EntryKind)>,
    /// The uploads and downloads of this pane, waiting, running and over.
    pub(crate) queue: Queue,
    /// An upload ended since the folders were last read again.
    pub(crate) uploaded: bool,
    /// The file being previewed, which then takes the place of the tree.
    pub(crate) preview: Option<Preview>,
    /// The context menu under a row, while it is open.
    pub(crate) menu: Option<Menu>,
    /// Whether the pane has a close button: not when it is the whole tab.
    pub(crate) closable: bool,
    /// True when the pane fills the whole tab instead of sitting beside the terminal.
    pub(crate) zoomed: bool,
    /// Whether rows show permissions and modification time.
    pub(crate) details: bool,
    /// An upload waiting for a yes, because its remote file exists: local file and remote path.
    pub(crate) pending_replace: Option<(PathBuf, String)>,
    /// The last problem or result, shown at the bottom.
    pub(crate) status: Option<Status>,
    /// The local pane shown beside this one, when there is one.
    pub(crate) other: Option<Box<State>>,
    /// Whether this pane is the local disk: it has no upload or download of its own.
    pub(crate) is_local: bool,
    /// A download ended since the other pane was last read again.
    pub(crate) downloaded: bool,
    /// Whether the keyboard is in the local pane rather than this one.
    pub(crate) focus_other: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Status {
    Failed(String),
}

/// The started file service on its way to the window.
#[derive(Clone)]
pub struct Ready {
    files: Arc<dyn RemoteFs>,
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
    UploadFolderPressed,
    /// A right-click on a row: its path and kind.
    MenuOpened(String, EntryKind),
    MenuChosen(Action),
    MenuTyped(String),
    /// Enter, or the confirm button of the menu.
    MenuSubmitted,
    MenuDismissed,
    /// A rename, new folder or delete ended: the folder to read again, or the reason it failed.
    OperationDone(Result<String, String>),
    PreviewPressed,
    /// The previewed file, fetched: its path and what it turned into.
    Previewed(String, Result<Content, String>),
    PreviewClosed,
    /// Where to save the downloaded folder, or `None` when the dialog was cancelled.
    DownloadFolderPicked(Option<PathBuf>),
    /// The files a folder stands for, ready to queue: local file and remote path.
    Expanded(Direction, Result<Vec<(PathBuf, String)>, String>),
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
    /// The "Zoom" button: fill the whole tab with the pane, or go back beside the terminal.
    ZoomToggled,
    /// The "This computer" button: show the local pane beside this one, or hide it.
    LocalToggled,
    /// The "Details" button: show or hide permissions and modification time.
    DetailsToggled,
    /// A message for the local pane.
    Other(Box<Message>),
    /// "Copy to this computer": the selection here goes into the local folder.
    CopyToLocalPressed,
    /// "Copy to the server": the local selection goes into the folder here.
    CopyToRemotePressed,
}

/// What the pane asks its session to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Close,
}

impl State {
    /// True for the narrow pane beside a terminal: icons for buttons, no second pane.
    pub(crate) fn is_compact(&self) -> bool {
        self.closable && !self.zoomed
    }

    /// Starts the file service on the session's connection.
    pub fn open(connection: Connection) -> (Self, Task<Message>) {
        Self::open_with(async move {
            // A server without SFTP is still reachable with scp and shell commands.
            let files: Arc<dyn RemoteFs> = match SftpFs::open(&connection).await {
                Ok(files) => Arc::new(files),
                Err(sftp_error) => match ScpFs::open(&connection).await {
                    Ok(files) => Arc::new(files),
                    Err(_) => return Err(sftp_error.to_string()),
                },
            };
            Ok(files)
        })
    }

    /// Shows the files of an already connected file system.
    pub fn browse(files: Arc<dyn RemoteFs>) -> (Self, Task<Message>) {
        Self::open_with(async move { Ok(files) })
    }

    fn open_with(
        connect: impl Future<Output = Result<Arc<dyn RemoteFs>, String>> + Send + 'static,
    ) -> (Self, Task<Message>) {
        let task = Task::perform(
            async move {
                let files = connect.await?;
                let home = files.home().await.map_err(|error| error.to_string())?;
                Ok(Ready { files, home })
            },
            Message::Opened,
        );
        (Self::empty(), task)
    }

    pub(crate) fn empty() -> Self {
        Self {
            files: None,
            root: String::new(),
            listings: HashMap::new(),
            expanded: HashSet::new(),
            selected: None,
            queue: Queue::default(),
            uploaded: false,
            preview: None,
            menu: None,
            closable: true,
            zoomed: false,
            details: true,
            pending_replace: None,
            status: None,
            other: None,
            is_local: false,
            downloaded: false,
            focus_other: false,
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
}

/// Whether a message is the user picking something in a pane, which gives that pane the keyboard.
fn is_a_pick(message: &Message) -> bool {
    matches!(
        message,
        Message::Selected(..) | Message::FolderToggled(_) | Message::MenuOpened(..)
    )
}

pub fn update(state: &mut State, message: Message) -> (Task<Message>, Option<Event>) {
    match &message {
        Message::Other(inner) if is_a_pick(inner) => state.focus_other = true,
        message if is_a_pick(message) => state.focus_other = false,
        _ => {}
    }
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
            state.menu = None;
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
            state.menu = None;
            state.selected = Some((path, kind));
            Task::none()
        }
        Message::UpPressed => match remotething_files::parent(&state.root) {
            Some(parent) => {
                state.menu = None;
                state.root = parent.clone();
                state.selected = None;
                state.list(&parent)
            }
            None => Task::none(),
        },
        Message::RefreshPressed => {
            state.menu = None;
            refresh(state)
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
        Message::DownloadPressed => match (state.selected_file(), state.selected_folder()) {
            (Some(path), _) => {
                let name = path.rsplit('/').next().unwrap_or(path).to_string();
                let dialog = file_dialog::save_any(fl!("files-download"), name);
                Task::perform(dialog, Message::DownloadPicked)
            }
            (None, Some(_)) => Task::perform(
                file_dialog::pick_folder(fl!("files-download-folder")),
                Message::DownloadFolderPicked,
            ),
            (None, None) => Task::none(),
        },
        Message::MenuOpened(path, kind) => {
            files_menu::open(state, path, kind);
            Task::none()
        }
        Message::MenuChosen(action) => files_menu::choose(state, action),
        Message::MenuTyped(text) => {
            files_menu::typed(state, text);
            Task::none()
        }
        Message::MenuSubmitted => files_menu::submit(state),
        Message::MenuDismissed => {
            state.menu = None;
            Task::none()
        }
        Message::OperationDone(result) => files_menu::done(state, result),
        Message::PreviewPressed => files_preview::open(state),
        Message::Previewed(path, result) => {
            files_preview::arrived(state, &path, result);
            Task::none()
        }
        Message::PreviewClosed => {
            state.preview = None;
            Task::none()
        }
        Message::UploadFolderPressed => Task::perform(
            file_dialog::pick_folder(fl!("files-upload-folder")),
            Message::UploadPicked,
        ),
        Message::DownloadFolderPicked(Some(destination)) => download_folder(state, destination),
        Message::Expanded(direction, Ok(pairs)) => {
            for (local, remote) in pairs {
                state.queue.add(direction, local, remote);
            }
            start_ready(state)
        }
        Message::Expanded(_, Err(reason)) => {
            state.status = Some(Status::Failed(reason));
            Task::none()
        }
        Message::DownloadPicked(Some(local)) => start_download(state, local),
        // A cancelled dialog.
        Message::UploadPicked(None)
        | Message::DownloadPicked(None)
        | Message::DownloadFolderPicked(None) => Task::none(),
        Message::TransferProgress(id, progress) => {
            state.queue.progress(id, progress);
            Task::none()
        }
        Message::TransferEnded(id, result) => {
            let was_upload = state
                .queue
                .job(id)
                .is_some_and(|job| job.direction == Direction::Upload && result.is_ok());
            let was_download = state
                .queue
                .job(id)
                .is_some_and(|job| job.direction == Direction::Download && result.is_ok());
            state.queue.ended(id, result);
            state.uploaded |= was_upload;
            state.downloaded |= was_download;
            // Once the last transfer is over, the folders show what was uploaded,
            // and the local pane what was downloaded.
            let mut refreshing = Vec::new();
            if state.uploaded && state.queue.is_idle() {
                state.uploaded = false;
                refreshing.push(refresh(state));
            }
            if state.downloaded && state.queue.is_idle() {
                state.downloaded = false;
                if let Some(other) = state.other.as_mut() {
                    refreshing.push(refresh(other).map(files_dual::wrap));
                }
            }
            refreshing.push(start_ready(state));
            Task::batch(refreshing)
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
        Message::ZoomToggled => {
            // Room for the columns comes with the zoom; a narrow pane has none to spare.
            state.zoomed = !state.zoomed;
            state.details = state.zoomed;
            Task::none()
        }
        Message::DetailsToggled => {
            state.details = !state.details;
            Task::none()
        }
        Message::LocalToggled => files_dual::toggle(state),
        Message::Other(inner) => match state.other.as_mut() {
            Some(other) => update(other, *inner).0.map(files_dual::wrap),
            None => Task::none(),
        },
        Message::CopyToLocalPressed => files_dual::copy_to_local(state),
        Message::CopyToRemotePressed => files_dual::copy_to_remote(state),
    };
    (task, None)
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
