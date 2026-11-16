//! SFTP over a channel of an open SSH connection.

use std::path::Path;

use remotething_remote::ssh::Connection;
use russh_sftp::client::SftpSession;

use crate::entry::{Entry, EntryKind, sort};
use crate::{Error, Result};

/// The files of an SSH server, reached through the connection of a session.
pub struct SftpFs {
    pub(crate) session: SftpSession,
}

impl SftpFs {
    /// Starts the file service on a new channel of `connection`.
    pub async fn open(connection: &Connection) -> Result<Self> {
        let stream = connection
            .open_sftp()
            .await
            .map_err(|error| Error::Unavailable(error.to_string()))?;
        let session = SftpSession::new(stream)
            .await
            .map_err(|error| Error::Unavailable(error.to_string()))?;
        Ok(Self { session })
    }

    /// The folder the server starts in, usually the user's home, as a full path.
    pub async fn home(&self) -> Result<String> {
        self.session
            .canonicalize(".")
            .await
            .map_err(|error| remote("cannot find the home folder", ".", error))
    }

    /// What is in a folder: folders first, then by name. `.` and `..` are left out.
    pub async fn list(&self, folder: &str) -> Result<Vec<Entry>> {
        let listing = self
            .session
            .read_dir(folder)
            .await
            .map_err(|error| remote("cannot list", folder, error))?;

        let mut entries: Vec<Entry> = listing
            .map(|item| {
                let attributes = item.metadata();
                let kind = if attributes.is_dir() {
                    EntryKind::Folder
                } else if attributes.is_symlink() {
                    EntryKind::Link
                } else if attributes.is_regular() {
                    EntryKind::File
                } else {
                    EntryKind::Other
                };
                Entry {
                    name: item.file_name(),
                    kind,
                    size: attributes.size,
                    modified: attributes.mtime.map(u64::from),
                    // Only the permission bits; the file type is in `kind`.
                    permissions: attributes.permissions.map(|bits| bits & 0o7777),
                }
            })
            .filter(|entry| entry.name != "." && entry.name != "..")
            .collect();
        sort(&mut entries);
        Ok(entries)
    }

    /// Whether something exists at a remote path.
    pub async fn exists(&self, path: &str) -> Result<bool> {
        self.session
            .try_exists(path)
            .await
            .map_err(|error| remote("cannot look at", path, error))
    }

    /// Makes one new remote folder. It fails when the path is already taken.
    pub async fn create_folder(&self, path: &str) -> Result<()> {
        self.session
            .create_dir(path)
            .await
            .map_err(|error| remote("cannot create", path, error))
    }

    /// Gives a remote file or folder a new path. It fails when the new path is already taken.
    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.session
            .rename(from, to)
            .await
            .map_err(|error| remote("cannot rename", from, error))
    }

    /// Deletes a remote file.
    pub async fn remove_file(&self, path: &str) -> Result<()> {
        self.session
            .remove_file(path)
            .await
            .map_err(|error| remote("cannot delete", path, error))
    }

    /// Deletes a remote folder. It fails when the folder is not empty.
    pub async fn remove_folder(&self, path: &str) -> Result<()> {
        self.session
            .remove_dir(path)
            .await
            .map_err(|error| remote("cannot delete", path, error))
    }

    /// Copies a remote file to a local one, replacing it. Returns the bytes copied.
    pub async fn download(&self, remote_file: &str, local_file: &Path) -> Result<u64> {
        let mut source = self
            .session
            .open(remote_file)
            .await
            .map_err(|error| remote("cannot open", remote_file, error))?;
        let mut target = tokio::fs::File::create(local_file)
            .await
            .map_err(|error| local(local_file, error))?;
        tokio::io::copy(&mut source, &mut target)
            .await
            .map_err(|error| remote("cannot download", remote_file, error))
    }

    /// Copies a local file to a remote one, replacing it. Returns the bytes copied.
    pub async fn upload(&self, local_file: &Path, remote_file: &str) -> Result<u64> {
        let mut source = tokio::fs::File::open(local_file)
            .await
            .map_err(|error| local(local_file, error))?;
        let mut target = self
            .session
            .create(remote_file)
            .await
            .map_err(|error| remote("cannot create", remote_file, error))?;
        let copied = tokio::io::copy(&mut source, &mut target)
            .await
            .map_err(|error| remote("cannot upload", remote_file, error))?;
        // Without this the last block can still be on its way when the caller lists the folder.
        tokio::io::AsyncWriteExt::shutdown(&mut target)
            .await
            .map_err(|error| remote("cannot finish", remote_file, error))?;
        Ok(copied)
    }
}

pub(crate) fn remote(action: &'static str, path: &str, error: impl std::fmt::Display) -> Error {
    Error::Remote {
        action,
        path: path.to_string(),
        reason: error.to_string(),
    }
}

pub(crate) fn local(path: &Path, error: std::io::Error) -> Error {
    Error::Local {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}
