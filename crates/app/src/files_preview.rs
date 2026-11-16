//! Fetching a remote file for the preview in the file pane.

use std::sync::Arc;

use iced::Task;

use crate::file_preview::{self, Content};
use crate::files_pane::{Message, State};

/// A preview that is open, or on its way.
pub(crate) struct Preview {
    pub name: String,
    pub path: String,
    /// `None` while the file is being fetched.
    pub content: Option<Result<Content, String>>,
}

/// Starts the preview of the selected file, if one is selected.
pub(crate) fn open(state: &mut State) -> Task<Message> {
    let (Some(files), Some(path)) = (state.files.clone(), state.selected_file()) else {
        return Task::none();
    };
    let path = path.to_string();
    let name = path.rsplit('/').next().unwrap_or(&path).to_string();
    state.status = None;
    state.preview = Some(Preview {
        name: name.clone(),
        path: path.clone(),
        content: None,
    });
    let fetched = {
        let path = path.clone();
        async move {
            let files = Arc::clone(&files);
            let (bytes, cut) = files
                .read_start(&path, file_preview::limit_for(&name))
                .await
                .map_err(|error| error.to_string())?;
            // Decoding a picture takes a moment; keep it off the window's thread.
            tokio::task::spawn_blocking(move || file_preview::decode(&name, &bytes, cut))
                .await
                .map_err(|error| error.to_string())
        }
    };
    Task::perform(fetched, move |result| {
        Message::Previewed(path.clone(), result)
    })
}

/// The fetched file arrived. It is dropped when the preview was closed or moved on meanwhile.
pub(crate) fn arrived(state: &mut State, path: &str, result: Result<Content, String>) {
    if let Some(preview) = state
        .preview
        .as_mut()
        .filter(|preview| preview.path == path)
    {
        preview.content = Some(result);
    }
}
