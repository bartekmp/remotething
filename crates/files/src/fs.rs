//! What every kind of remote file system can do.

use std::path::Path;

use async_trait::async_trait;

use crate::{Entry, EntryKind, Progress, Result, Transferred};

/// A way to report how far a transfer has come.
pub type OnProgress<'a> = &'a mut (dyn FnMut(Progress) + Send);

/// A remote file system: SFTP, and later FTP and others. Paths are remote
/// paths, written with `/`.
#[async_trait]
pub trait RemoteFs: Send + Sync {
    /// The folder the server starts in, usually the user's home, as a full path.
    async fn home(&self) -> Result<String>;

    /// What is in a folder: folders first, then by name. `.` and `..` are left out.
    async fn list(&self, folder: &str) -> Result<Vec<Entry>>;

    /// Whether something exists at a remote path.
    async fn exists(&self, path: &str) -> Result<bool>;

    /// The size of a remote file.
    async fn size(&self, path: &str) -> Result<u64>;

    /// Makes one new remote folder. It fails when the path is already taken.
    async fn create_folder(&self, path: &str) -> Result<()>;

    /// Makes a remote folder and the ones above it that are missing.
    async fn create_folders(&self, folder: &str) -> Result<()>;

    /// Gives a remote file or folder a new path. It fails when the new path is already taken.
    async fn rename(&self, from: &str, to: &str) -> Result<()>;

    /// Deletes a remote file.
    async fn remove_file(&self, path: &str) -> Result<()>;

    /// Deletes a remote folder. It fails when the folder is not empty.
    async fn remove_folder(&self, path: &str) -> Result<()>;

    /// The first `limit` bytes of a remote file, read into memory, and whether the file goes on.
    async fn read_start(&self, path: &str, limit: u64) -> Result<(Vec<u8>, bool)>;

    /// Copies a remote file to a local one, reporting progress.
    ///
    /// With `resume`, a local file that is shorter than the remote one is taken
    /// as the start of it and only the rest is fetched. Otherwise the local file is replaced.
    async fn download_tracked(
        &self,
        remote_file: &str,
        local_file: &Path,
        resume: bool,
        progress: OnProgress<'_>,
    ) -> Result<Transferred>;

    /// Copies a local file to a remote one, reporting progress.
    ///
    /// With `resume`, a remote file that is shorter than the local one is taken
    /// as the start of it and only the rest is sent. Otherwise the remote file is replaced.
    async fn upload_tracked(
        &self,
        local_file: &Path,
        remote_file: &str,
        resume: bool,
        progress: OnProgress<'_>,
    ) -> Result<Transferred>;

    /// Every file under a remote folder, with its size, as paths relative to the folder
    /// (`a/b.txt`). Links and other special entries are left out, so a loop cannot trap the walk.
    async fn files_under(&self, folder: &str) -> Result<Vec<(String, u64)>> {
        let mut found = Vec::new();
        let mut waiting = vec![String::new()];
        while let Some(relative) = waiting.pop() {
            let here = if relative.is_empty() {
                folder.to_string()
            } else {
                crate::join(folder, &relative)
            };
            for entry in self.list(&here).await? {
                let name = if relative.is_empty() {
                    entry.name.clone()
                } else {
                    format!("{relative}/{}", entry.name)
                };
                match entry.kind {
                    EntryKind::Folder => waiting.push(name),
                    EntryKind::File => found.push((name, entry.size.unwrap_or(0))),
                    _ => {}
                }
            }
        }
        found.sort();
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::Entry;

    /// Folders and their entries, and nothing else: enough to walk.
    struct Listing(HashMap<String, Vec<Entry>>);

    fn entry(name: &str, kind: EntryKind, size: u64) -> Entry {
        Entry {
            name: name.to_string(),
            kind,
            size: Some(size),
            modified: None,
            permissions: None,
        }
    }

    #[async_trait]
    impl RemoteFs for Listing {
        async fn list(&self, folder: &str) -> Result<Vec<Entry>> {
            Ok(self.0.get(folder).cloned().unwrap_or_default())
        }
        async fn home(&self) -> Result<String> {
            unimplemented!()
        }
        async fn exists(&self, _: &str) -> Result<bool> {
            unimplemented!()
        }
        async fn size(&self, _: &str) -> Result<u64> {
            unimplemented!()
        }
        async fn create_folder(&self, _: &str) -> Result<()> {
            unimplemented!()
        }
        async fn create_folders(&self, _: &str) -> Result<()> {
            unimplemented!()
        }
        async fn rename(&self, _: &str, _: &str) -> Result<()> {
            unimplemented!()
        }
        async fn remove_file(&self, _: &str) -> Result<()> {
            unimplemented!()
        }
        async fn remove_folder(&self, _: &str) -> Result<()> {
            unimplemented!()
        }
        async fn read_start(&self, _: &str, _: u64) -> Result<(Vec<u8>, bool)> {
            unimplemented!()
        }
        async fn download_tracked(
            &self,
            _: &str,
            _: &Path,
            _: bool,
            _: OnProgress<'_>,
        ) -> Result<Transferred> {
            unimplemented!()
        }
        async fn upload_tracked(
            &self,
            _: &Path,
            _: &str,
            _: bool,
            _: OnProgress<'_>,
        ) -> Result<Transferred> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn the_walk_finds_files_at_every_depth_and_skips_links() {
        let mut folders = HashMap::new();
        folders.insert(
            "/srv".to_string(),
            vec![
                entry("a", EntryKind::Folder, 0),
                entry("top.txt", EntryKind::File, 1),
                entry("loop", EntryKind::Link, 0),
            ],
        );
        folders.insert(
            "/srv/a".to_string(),
            vec![entry("deep.txt", EntryKind::File, 2)],
        );

        let found = Listing(folders).files_under("/srv").await.unwrap();

        assert_eq!(
            found,
            [("a/deep.txt".to_string(), 2), ("top.txt".to_string(), 1)]
        );
    }
}
