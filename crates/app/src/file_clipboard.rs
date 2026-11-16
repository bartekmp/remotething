//! Files on the system clipboard: what Dolphin, Explorer or Finder put there on "Copy".

use std::path::PathBuf;

use crate::image_clipboard::with_clipboard;

/// The files on the clipboard, if it holds some. This can block for a moment.
pub(crate) fn read() -> Option<Vec<PathBuf>> {
    with_clipboard(|clipboard| clipboard.get().file_list().ok()).filter(|files| !files.is_empty())
}
