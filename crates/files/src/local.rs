//! The disk of this computer, as a file system like the remote ones.
//!
//! It lets the same file pane browse local folders, and lets a copy between
//! the two panes go through the transfer queue like any other.

use std::io::SeekFrom;
use std::path::Path;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::entry::{Entry, EntryKind, sort};
use crate::fs::{OnProgress, RemoteFs};
use crate::transfer::{Failure, Transferred, copy};
use crate::{Error, Result};

/// The local disk. Paths are written with `/`, also on Windows.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalFs;

#[async_trait]
impl RemoteFs for LocalFs {
    async fn home(&self) -> Result<String> {
        let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let home = std::env::var(variable).unwrap_or_else(|_| "/".to_string());
        Ok(home.replace('\\', "/"))
    }

    async fn list(&self, folder: &str) -> Result<Vec<Entry>> {
        let mut reader = tokio::fs::read_dir(folder)
            .await
            .map_err(|error| fail("cannot list", folder, error))?;
        let mut entries = Vec::new();
        while let Some(item) = reader
            .next_entry()
            .await
            .map_err(|error| fail("cannot list", folder, error))?
        {
            // A file that vanishes between the listing and the lookup is simply skipped.
            let Ok(metadata) = tokio::fs::symlink_metadata(item.path()).await else {
                continue;
            };
            let kind = if metadata.is_dir() {
                EntryKind::Folder
            } else if metadata.file_type().is_symlink() {
                EntryKind::Link
            } else if metadata.is_file() {
                EntryKind::File
            } else {
                EntryKind::Other
            };
            entries.push(Entry {
                name: item.file_name().to_string_lossy().into_owned(),
                kind,
                size: (kind == EntryKind::File).then_some(metadata.len()),
                modified: modified_seconds(&metadata),
                permissions: permission_bits(&metadata),
            });
        }
        sort(&mut entries);
        Ok(entries)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        tokio::fs::try_exists(path)
            .await
            .map_err(|error| fail("cannot look at", path, error))
    }

    async fn size(&self, path: &str) -> Result<u64> {
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|error| fail("cannot look at", path, error))?;
        Ok(metadata.len())
    }

    async fn create_folder(&self, path: &str) -> Result<()> {
        tokio::fs::create_dir(path)
            .await
            .map_err(|error| fail("cannot create", path, error))
    }

    async fn create_folders(&self, folder: &str) -> Result<()> {
        tokio::fs::create_dir_all(folder)
            .await
            .map_err(|error| fail("cannot create", folder, error))
    }

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        // The operating system would silently replace a file that is there.
        if self.exists(to).await? {
            return Err(Error::Remote {
                action: "cannot rename",
                path: to.to_string(),
                reason: "the name is taken".to_string(),
            });
        }
        tokio::fs::rename(from, to)
            .await
            .map_err(|error| fail("cannot rename", from, error))
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        tokio::fs::remove_file(path)
            .await
            .map_err(|error| fail("cannot delete", path, error))
    }

    async fn remove_folder(&self, path: &str) -> Result<()> {
        tokio::fs::remove_dir(path)
            .await
            .map_err(|error| fail("cannot delete", path, error))
    }

    async fn read_start(&self, path: &str, limit: u64) -> Result<(Vec<u8>, bool)> {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|error| fail("cannot open", path, error))?;
        let mut data = Vec::new();
        // One byte more than asked tells whether the file goes on.
        file.take(limit + 1)
            .read_to_end(&mut data)
            .await
            .map_err(|error| fail("cannot read", path, error))?;
        let more = data.len() as u64 > limit;
        data.truncate(limit as usize);
        Ok((data, more))
    }

    async fn download_tracked(
        &self,
        remote_file: &str,
        local_file: &Path,
        resume: bool,
        progress: OnProgress<'_>,
    ) -> Result<Transferred> {
        copy_file(Path::new(remote_file), local_file, resume, progress).await
    }

    async fn upload_tracked(
        &self,
        local_file: &Path,
        remote_file: &str,
        resume: bool,
        progress: OnProgress<'_>,
    ) -> Result<Transferred> {
        if let Some(folder) = crate::parent(remote_file) {
            self.create_folders(&folder).await?;
        }
        copy_file(local_file, Path::new(remote_file), resume, progress).await
    }
}

/// Copies one file to another place on the disk, carrying on from a shorter copy when asked.
async fn copy_file(
    from: &Path,
    to: &Path,
    resume: bool,
    progress: OnProgress<'_>,
) -> Result<Transferred> {
    let mut source = tokio::fs::File::open(from)
        .await
        .map_err(|error| local(from, error))?;
    let total = source
        .metadata()
        .await
        .map_err(|error| local(from, error))?
        .len();
    let offset = if resume {
        match tokio::fs::metadata(to).await {
            Ok(existing) if existing.len() > 0 && existing.len() <= total => existing.len(),
            _ => 0,
        }
    } else {
        0
    };

    let mut target = if offset > 0 {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(to)
            .await
            .map_err(|error| local(to, error))?;
        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(|error| local(to, error))?;
        source
            .seek(SeekFrom::Start(offset))
            .await
            .map_err(|error| local(from, error))?;
        file
    } else {
        tokio::fs::File::create(to)
            .await
            .map_err(|error| local(to, error))?
    };

    let done = copy(&mut source, &mut target, offset, total, progress)
        .await
        .map_err(|failure| match failure {
            Failure::Reading(error) => local(from, error),
            Failure::Writing(error) => local(to, error),
        })?;
    target.flush().await.map_err(|error| local(to, error))?;
    Ok(Transferred {
        bytes: done,
        resumed_from: offset,
    })
}

fn modified_seconds(metadata: &std::fs::Metadata) -> Option<u64> {
    let modified = metadata.modified().ok()?;
    let since = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(since.as_secs())
}

#[cfg(unix)]
fn permission_bits(metadata: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(metadata.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn permission_bits(_: &std::fs::Metadata) -> Option<u32> {
    None
}

fn fail(action: &'static str, path: &str, error: std::io::Error) -> Error {
    Error::Remote {
        action,
        path: path.to_string(),
        reason: error.to_string(),
    }
}

fn local(path: &Path, error: std::io::Error) -> Error {
    Error::Local {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    #[tokio::test]
    async fn listing_puts_folders_first_and_reports_sizes() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("b.txt"), "12345").unwrap();
        std::fs::create_dir(directory.path().join("zdir")).unwrap();

        let entries = LocalFs.list(&text(directory.path())).await.unwrap();

        assert_eq!(entries[0].name, "zdir");
        assert_eq!(entries[0].kind, EntryKind::Folder);
        assert_eq!(entries[1].name, "b.txt");
        assert_eq!(entries[1].size, Some(5));
    }

    #[tokio::test]
    async fn a_missing_folder_is_an_error_naming_it() {
        let error = LocalFs.list("/no/such/folder").await.unwrap_err();

        assert!(error.to_string().contains("/no/such/folder"), "{error}");
    }

    #[tokio::test]
    async fn rename_refuses_a_taken_name() {
        let directory = tempfile::tempdir().unwrap();
        let a = text(&directory.path().join("a"));
        let b = text(&directory.path().join("b"));
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();

        assert!(LocalFs.rename(&a, &b).await.is_err());
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "b");
    }

    #[tokio::test]
    async fn read_start_says_whether_the_file_goes_on() {
        let directory = tempfile::tempdir().unwrap();
        let file = text(&directory.path().join("f"));
        std::fs::write(&file, "0123456789").unwrap();

        assert_eq!(
            LocalFs.read_start(&file, 4).await.unwrap(),
            (b"0123".to_vec(), true)
        );
        assert_eq!(
            LocalFs.read_start(&file, 10).await.unwrap(),
            (b"0123456789".to_vec(), false)
        );
    }

    #[tokio::test]
    async fn an_upload_makes_missing_folders_and_a_resume_finishes_a_short_copy() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        std::fs::write(&source, "0123456789").unwrap();
        let target = text(&directory.path().join("a/b/target"));

        let first = LocalFs
            .upload_tracked(&source, &target, false, &mut |_| {})
            .await
            .unwrap();
        assert_eq!(first.bytes, 10);

        std::fs::write(&target, "0123").unwrap();
        let second = LocalFs
            .upload_tracked(&source, &target, true, &mut |_| {})
            .await
            .unwrap();

        assert_eq!(second.resumed_from, 4);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "0123456789");
    }

    #[tokio::test]
    async fn a_download_without_resume_replaces_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let from = text(&directory.path().join("from"));
        let to = directory.path().join("to");
        std::fs::write(&from, "new").unwrap();
        std::fs::write(&to, "old content").unwrap();

        LocalFs
            .download_tracked(&from, &to, false, &mut |_| {})
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(&to).unwrap(), "new");
    }
}
