//! Files going between a remote desktop and this machine, over the clipboard channel.
//!
//! To send: pick files or drop them on the window, then paste on the remote
//! desktop. To receive: copy files there, then press the button and pick a folder.

use std::path::PathBuf;
use std::time::Duration;

use iced::Task;
use remotething_rdp::{Direction, FilesEvent, Input, RemoteFile};

use crate::desktop::{Message as DesktopMessage, Stage, State};
use crate::file_dialog;
use crate::files_pane::format_size;
use crate::i18n::fl;
use crate::note::Note;

/// Files dropped one after another are offered together once none has come for this long.
const DROP_SETTLES: Duration = Duration::from_millis(250);

/// What is known about the files of a desktop.
#[derive(Debug, Default)]
pub(crate) struct Files {
    /// What the server copied last, which can be received.
    available: Option<Vec<RemoteFile>>,
    /// Which way a transfer is going, and how far it has come.
    progress: Option<(Direction, u64, u64)>,
    /// Files dropped on the window, waiting for the rest of the drop.
    dropped: Vec<PathBuf>,
}

impl Files {
    /// What the strip says while files are moving.
    pub(crate) fn status(&self) -> Option<String> {
        let (direction, done, total) = self.progress?;
        let (done, total) = (format_size(done), format_size(total));
        Some(match direction {
            Direction::Received => fl!("rdpfiles-progress-received", done = done, total = total),
            Direction::Sent => fl!("rdpfiles-progress-sent", done = done, total = total),
        })
    }

    /// How much of the files being received has arrived, from 0 to 1. Sending shows no bar:
    /// the server asks for the bytes as it pastes, and Windows shows its own progress for that.
    pub(crate) fn progress_share(&self) -> Option<f32> {
        match self.progress {
            Some((Direction::Received, done, total)) if total > 0 => {
                Some(done as f32 / total as f32)
            }
            _ => None,
        }
    }

    /// The buttons for files, in the order they are shown.
    pub(crate) fn actions(&self) -> Vec<(String, DesktopMessage)> {
        let mut actions = Vec::new();
        let receiving = matches!(self.progress, Some((Direction::Received, ..)));
        if receiving {
            actions.push((fl!("files-cancel"), wrap(Message::CancelPressed)));
        } else if let Some(count) = self.receivable() {
            actions.push((
                fl!("rdpfiles-receive", count = count),
                wrap(Message::ReceivePressed),
            ));
        }
        actions.push((fl!("rdpfiles-send"), wrap(Message::SendPressed)));
        actions
    }

    /// How many files, not folders, the server copied; `None` when it copied none.
    fn receivable(&self) -> Option<usize> {
        let files = self.available.as_ref()?;
        let count = files.iter().filter(|file| !file.is_folder).count();
        (count > 0).then_some(count)
    }
}

fn wrap(message: Message) -> DesktopMessage {
    DesktopMessage::Files(message)
}

#[derive(Debug, Clone)]
pub enum Message {
    /// The session says something happened to files.
    Event(FilesEvent),
    SendPressed,
    /// The files picked to send, or `None` when the dialog was cancelled.
    Picked(Option<Vec<PathBuf>>),
    ReceivePressed,
    /// The folder picked to receive into, or `None` when the dialog was cancelled.
    FolderPicked(Option<PathBuf>),
    CancelPressed,
    /// A file or folder was dropped on the window.
    Dropped(PathBuf),
    /// No more came after a drop, so what came is offered.
    DropSettled,
}

pub(crate) fn update(state: &mut State, message: Message) -> Task<DesktopMessage> {
    match message {
        Message::Event(event) => {
            on_event(state, event);
            Task::none()
        }
        Message::SendPressed => Task::perform(
            file_dialog::open_many(fl!("rdpfiles-pick-send")),
            |picked| wrap(Message::Picked(picked)),
        ),
        Message::Picked(Some(paths)) => {
            state.send(Input::OfferFiles(paths));
            Task::none()
        }
        Message::ReceivePressed => Task::perform(
            file_dialog::pick_folder(fl!("rdpfiles-pick-folder")),
            |folder| wrap(Message::FolderPicked(folder)),
        ),
        Message::FolderPicked(Some(folder)) => {
            state.send(Input::ReceiveFiles(folder));
            Task::none()
        }
        // A cancelled dialog.
        Message::Picked(None) | Message::FolderPicked(None) => Task::none(),
        Message::CancelPressed => {
            state.send(Input::CancelFiles);
            Task::none()
        }
        Message::Dropped(path) => {
            let Stage::Connected(live) = &mut state.stage else {
                return Task::none();
            };
            let first = live.files.dropped.is_empty();
            live.files.dropped.push(path);
            if !first {
                return Task::none();
            }
            Task::perform(tokio::time::sleep(DROP_SETTLES), |()| {
                wrap(Message::DropSettled)
            })
        }
        Message::DropSettled => {
            let Stage::Connected(live) = &mut state.stage else {
                return Task::none();
            };
            let paths = std::mem::take(&mut live.files.dropped);
            if !paths.is_empty() {
                state.send(Input::OfferFiles(paths));
            }
            Task::none()
        }
    }
}

/// Remembers what happened and tells the user in the strip.
fn on_event(state: &mut State, event: FilesEvent) {
    let Stage::Connected(live) = &mut state.stage else {
        return;
    };
    let files = &mut live.files;
    let say = match event {
        FilesEvent::Available(list) => {
            files.available = Some(list);
            files
                .receivable()
                .map(|count| fl!("rdpfiles-available", count = count))
        }
        FilesEvent::Offered { files: count, .. } => Some(fl!("rdpfiles-offered", count = count)),
        FilesEvent::Progress {
            direction,
            done,
            total,
        } => {
            files.progress = Some((direction, done, total));
            None
        }
        FilesEvent::Finished {
            direction,
            files: count,
        } => {
            files.progress = None;
            Some(match direction {
                Direction::Received => fl!("rdpfiles-finished-received", count = count),
                Direction::Sent => fl!("rdpfiles-finished-sent", count = count),
            })
        }
        FilesEvent::Failed { reason, .. } => {
            files.progress = None;
            Some(reason)
        }
    };
    if let Some(text) = say {
        live.note = Some(Note::new(text));
    }
}

#[cfg(test)]
#[path = "desktop_files_tests.rs"]
mod tests;
