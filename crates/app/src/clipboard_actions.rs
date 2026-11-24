//! This computer's clipboard, offered to the remote desktop in front.
//!
//! One step at a time: files first, then text, then a picture.

use iced::Task;

use crate::app::{Message, State};
use crate::file_clipboard;
use crate::image_clipboard;

/// Starts a look at the clipboard.
///
/// Files first: copying them in a file manager often puts their paths there as text too.
pub(crate) fn on_tick() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(file_clipboard::read)
                .await
                .ok()
                .flatten()
        },
        Message::ClipboardFilesRead,
    )
}

/// Offers the files to the desktop, or goes on to the text.
pub(crate) fn on_files_read(
    state: &mut State,
    files: Option<Vec<std::path::PathBuf>>,
) -> Task<Message> {
    let offered = files.is_some_and(|files| state.tabs.desktop_clipboard_files(files));
    if offered {
        Task::none()
    } else {
        iced::clipboard::read().map(Message::ClipboardRead)
    }
}

/// Offers the text to the desktop, or goes on to a picture.
pub(crate) fn on_text_read(state: &mut State, text: Option<String>) -> Task<Message> {
    match text.filter(|text| !text.is_empty()) {
        Some(text) => {
            state.tabs.desktop_clipboard(text);
            Task::none()
        }
        // No text: there may be a picture.
        None => Task::perform(
            async {
                tokio::task::spawn_blocking(image_clipboard::read)
                    .await
                    .ok()
                    .flatten()
            },
            Message::ClipboardPictureRead,
        ),
    }
}

/// Offers the picture to the desktop.
pub(crate) fn on_picture_read(state: &mut State, picture: Option<remotething_rdp::Picture>) {
    if let Some(picture) = picture {
        state.tabs.desktop_clipboard_picture(picture);
    }
}
