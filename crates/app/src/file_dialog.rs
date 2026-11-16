//! The system's file dialogs. The only module that knows which library shows them.

use std::path::PathBuf;

/// Asks where to save a file. `None` when the user cancels.
pub async fn save(title: String, suggested_name: String) -> Option<PathBuf> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title(title)
        .set_file_name(suggested_name)
        .add_filter("JSON", &["json"])
        .save_file()
        .await?;
    Some(picked.path().to_path_buf())
}

/// Asks which file to open. `None` when the user cancels.
pub async fn open(title: String) -> Option<PathBuf> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title(title)
        .add_filter("JSON", &["json"])
        .pick_file()
        .await?;
    Some(picked.path().to_path_buf())
}

/// Asks where to save a file of any kind. `None` when the user cancels.
pub async fn save_any(title: String, suggested_name: String) -> Option<PathBuf> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title(title)
        .set_file_name(suggested_name)
        .save_file()
        .await?;
    Some(picked.path().to_path_buf())
}

/// Asks which file of any kind to open. `None` when the user cancels.
pub async fn open_any(title: String) -> Option<PathBuf> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title(title)
        .pick_file()
        .await?;
    Some(picked.path().to_path_buf())
}

/// Asks which folder to use. `None` when the user cancels.
pub async fn pick_folder(title: String) -> Option<PathBuf> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title(title)
        .pick_folder()
        .await?;
    Some(picked.path().to_path_buf())
}
