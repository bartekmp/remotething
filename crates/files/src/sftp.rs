//! SFTP over a channel of an open SSH connection.

use std::io::SeekFrom;
use std::path::Path;

use async_trait::async_trait;
use remotething_remote::ssh::Connection;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::entry::{Entry, EntryKind, sort};
use crate::fs::{OnProgress, RemoteFs};
use crate::transfer::{Failure, Transferred, copy};
use crate::{Error, Result};

/// The files of an SSH server, reached through the connection of a session.
pub struct SftpFs {
    session: SftpSession,
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
}

#[async_trait]
impl RemoteFs for SftpFs {
    async fn home(&self) -> Result<String> {
        self.session
            .canonicalize(".")
            .await
            .map_err(|error| remote("cannot find the home folder", ".", error))
    }

    async fn list(&self, folder: &str) -> Result<Vec<Entry>> {
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

    async fn exists(&self, path: &str) -> Result<bool> {
        self.session
            .try_exists(path)
            .await
            .map_err(|error| remote("cannot look at", path, error))
    }

    async fn size(&self, path: &str) -> Result<u64> {
        self.session
            .metadata(path)
            .await
            .map(|metadata| metadata.size.unwrap_or(0))
            .map_err(|error| remote("cannot look at", path, error))
    }

    async fn create_folder(&self, path: &str) -> Result<()> {
        self.session
            .create_dir(path)
            .await
            .map_err(|error| remote("cannot create", path, error))
    }

    async fn create_folders(&self, folder: &str) -> Result<()> {
        let mut path = String::new();
        for part in folder.split('/').filter(|part| !part.is_empty()) {
            path.push('/');
            path.push_str(part);
            if !self.exists(&path).await? {
                self.create_folder(&path).await?;
            }
        }
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.session
            .rename(from, to)
            .await
            .map_err(|error| remote("cannot rename", from, error))
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        self.session
            .remove_file(path)
            .await
            .map_err(|error| remote("cannot delete", path, error))
    }

    async fn remove_folder(&self, path: &str) -> Result<()> {
        self.session
            .remove_dir(path)
            .await
            .map_err(|error| remote("cannot delete", path, error))
    }

    async fn read_start(&self, path: &str, limit: u64) -> Result<(Vec<u8>, bool)> {
        let mut source = self
            .session
            .open(path)
            .await
            .map_err(|error| remote("cannot open", path, error))?;
        // One byte more than asked for tells whether the file is longer.
        let mut bytes = Vec::new();
        (&mut source)
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| remote("cannot read", path, error))?;
        let longer = bytes.len() as u64 > limit;
        bytes.truncate(limit as usize);
        Ok((bytes, longer))
    }

    async fn download_tracked(
        &self,
        remote_file: &str,
        local_file: &Path,
        resume: bool,
        progress: OnProgress<'_>,
    ) -> Result<Transferred> {
        let total = self.size(remote_file).await?;
        let offset = if resume {
            match tokio::fs::metadata(local_file).await {
                Ok(metadata) if total > 0 && metadata.len() <= total => metadata.len(),
                _ => 0,
            }
        } else {
            0
        };

        if let Some(folder) = local_file.parent() {
            tokio::fs::create_dir_all(folder)
                .await
                .map_err(|error| local(folder, error))?;
        }
        let mut source = self
            .session
            .open(remote_file)
            .await
            .map_err(|error| remote("cannot open", remote_file, error))?;
        let mut options = tokio::fs::OpenOptions::new();
        if offset > 0 {
            source
                .seek(SeekFrom::Start(offset))
                .await
                .map_err(|error| remote("cannot continue", remote_file, error))?;
            options.append(true);
        } else {
            options.write(true).create(true).truncate(true);
        }
        let mut target = options
            .open(local_file)
            .await
            .map_err(|error| local(local_file, error))?;

        let done = copy(&mut source, &mut target, offset, total, progress)
            .await
            .map_err(|failure| match failure {
                Failure::Reading(error) => remote("cannot download", remote_file, error),
                Failure::Writing(error) => local(local_file, error),
            })?;
        target
            .flush()
            .await
            .map_err(|error| local(local_file, error))?;
        Ok(Transferred {
            bytes: done,
            resumed_from: offset,
        })
    }

    async fn upload_tracked(
        &self,
        local_file: &Path,
        remote_file: &str,
        resume: bool,
        progress: OnProgress<'_>,
    ) -> Result<Transferred> {
        let mut source = tokio::fs::File::open(local_file)
            .await
            .map_err(|error| local(local_file, error))?;
        let total = source
            .metadata()
            .await
            .map_err(|error| local(local_file, error))?
            .len();
        let offset = if resume {
            match self.size(remote_file).await {
                Ok(size) if size > 0 && size <= total => size,
                _ => 0,
            }
        } else {
            0
        };

        if let Some(folder) = crate::parent(remote_file) {
            self.create_folders(&folder).await?;
        }
        let mut target = if offset > 0 {
            let mut file = self
                .session
                .open_with_flags(remote_file, OpenFlags::WRITE)
                .await
                .map_err(|error| remote("cannot open", remote_file, error))?;
            file.seek(SeekFrom::Start(offset))
                .await
                .map_err(|error| remote("cannot continue", remote_file, error))?;
            file
        } else {
            self.session
                .create(remote_file)
                .await
                .map_err(|error| remote("cannot create", remote_file, error))?
        };
        if offset > 0 {
            source
                .seek(SeekFrom::Start(offset))
                .await
                .map_err(|error| local(local_file, error))?;
        }

        let done = copy(&mut source, &mut target, offset, total, progress)
            .await
            .map_err(|failure| match failure {
                Failure::Reading(error) => local(local_file, error),
                Failure::Writing(error) => remote("cannot upload", remote_file, error),
            })?;
        // Without this the last block can still be on its way when the caller lists the folder.
        target
            .shutdown()
            .await
            .map_err(|error| remote("cannot finish", remote_file, error))?;
        Ok(Transferred {
            bytes: done,
            resumed_from: offset,
        })
    }
}

fn remote(action: &'static str, path: &str, error: impl std::fmt::Display) -> Error {
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
