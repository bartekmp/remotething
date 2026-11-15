//! Saving what a session shows: a picture for a remote desktop, the text on
//! the screen for a terminal.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use crate::capture_files::{create_file, create_private_folder, stamped_name, unix_now};
use crate::i18n::fl;
use crate::note::Note;

/// Saves a picture of `width` by `height` RGBA pixels as a PNG in `folder`.
/// The answer is the name of the file, or what went wrong.
pub(crate) async fn save_picture(
    folder: PathBuf,
    connection: String,
    width: u32,
    height: u32,
    rgba: Arc<Vec<u8>>,
) -> Result<String, String> {
    // Encoding and writing take a moment, so they stay off the UI thread.
    tokio::task::spawn_blocking(move || {
        let png = remotething_capture::png::encode(width, height, &rgba)?;
        write(&folder, &connection, "png", &png)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Saves the text on a terminal's screen in `folder`.
pub(crate) async fn save_text(
    folder: PathBuf,
    connection: String,
    text: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || write(&folder, &connection, "txt", text.as_bytes()))
        .await
        .map_err(|error| error.to_string())?
}

/// The note that tells how a screenshot went.
pub(crate) fn note_for(result: Result<String, String>) -> Note {
    Note::new(match result {
        Ok(name) => fl!("screenshot-saved", name = name),
        Err(reason) => format!("{} {reason}", fl!("screenshot-failed")),
    })
}

fn write(
    folder: &std::path::Path,
    connection: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<String, String> {
    create_private_folder(folder).map_err(|error| error.to_string())?;
    let name = stamped_name(connection, unix_now(), extension);
    let (mut file, path) = create_file(folder, &name).map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    Ok(path
        .file_name()
        .map_or(name, |name| name.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_picture_is_saved_as_a_png_in_the_folder() {
        let directory = tempfile::tempdir().unwrap();

        let name = save_picture(
            directory.path().join("shots"),
            "win 11".to_string(),
            1,
            1,
            Arc::new(vec![1, 2, 3, 255]),
        )
        .await
        .unwrap();

        assert!(
            name.starts_with("win-11-") && name.ends_with(".png"),
            "{name}"
        );
        let bytes = std::fs::read(directory.path().join("shots").join(&name)).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
    }

    #[tokio::test]
    async fn a_picture_that_does_not_fit_its_size_is_reported() {
        let directory = tempfile::tempdir().unwrap();

        let result = save_picture(
            directory.path().to_path_buf(),
            "x".to_string(),
            2,
            2,
            Arc::new(vec![0; 4]),
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn the_text_of_a_screen_is_saved_as_it_is() {
        let directory = tempfile::tempdir().unwrap();

        let name = save_text(
            directory.path().to_path_buf(),
            "web-01".to_string(),
            "hello\nworld\n".to_string(),
        )
        .await
        .unwrap();

        assert!(name.ends_with(".txt"));
        let text = std::fs::read_to_string(directory.path().join(name)).unwrap();
        assert_eq!(text, "hello\nworld\n");
    }
}
