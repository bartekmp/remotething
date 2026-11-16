//! What the file pane can tell about its tree and selection.

use remotething_files::EntryKind;

use crate::files_pane::{Listing, Row, State};

impl State {
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

    /// The selected folder, if a folder is selected.
    pub(crate) fn selected_folder(&self) -> Option<&str> {
        match &self.selected {
            Some((path, EntryKind::Folder)) => Some(path),
            _ => None,
        }
    }

    /// Whether something is selected that can be downloaded: a file or a folder.
    pub(crate) fn can_download(&self) -> bool {
        self.selected_file().is_some() || self.selected_folder().is_some()
    }

    /// The selected path, when it is something that can be downloaded.
    pub(crate) fn selected_file(&self) -> Option<&str> {
        match &self.selected {
            Some((path, EntryKind::File | EntryKind::Link)) => Some(path),
            _ => None,
        }
    }
}
