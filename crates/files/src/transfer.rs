//! Moving files with progress, and carrying on a transfer that was cut short.

use std::io::SeekFrom;
use std::path::Path;

use russh_sftp::protocol::OpenFlags;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::Result;
use crate::sftp::SftpFs;

/// How many bytes are moved before progress is reported again.
const CHUNK: usize = 64 * 1024;

/// How far a transfer has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Bytes of the file that are in place, including those from an earlier try.
    pub done: u64,
    /// The size of the whole file; 0 when the server did not say.
    pub total: u64,
}

/// What a finished transfer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transferred {
    /// The size of the file now.
    pub bytes: u64,
    /// Where the transfer took up an earlier try; 0 when it started from the beginning.
    pub resumed_from: u64,
}

impl SftpFs {
    /// Makes a remote folder and the ones above it that are missing.
    pub async fn create_folders(&self, folder: &str) -> Result<()> {
        let mut path = String::new();
        for part in folder.split('/').filter(|part| !part.is_empty()) {
            path.push('/');
            path.push_str(part);
            let exists = self
                .session
                .try_exists(path.clone())
                .await
                .map_err(|error| crate::sftp::remote("cannot look at", &path, error))?;
            if !exists {
                self.session
                    .create_dir(path.clone())
                    .await
                    .map_err(|error| crate::sftp::remote("cannot create", &path, error))?;
            }
        }
        Ok(())
    }

    /// Every file under a remote folder, with its size, as paths relative to the folder
    /// (`a/b.txt`). Links and other special entries are left out, so a loop cannot trap the walk.
    pub async fn files_under(&self, folder: &str) -> Result<Vec<(String, u64)>> {
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
                    crate::EntryKind::Folder => waiting.push(name),
                    crate::EntryKind::File => found.push((name, entry.size.unwrap_or(0))),
                    _ => {}
                }
            }
        }
        found.sort();
        Ok(found)
    }

    /// The first `limit` bytes of a remote file, read into memory, and whether the file goes on.
    pub async fn read_start(&self, remote_file: &str, limit: u64) -> Result<(Vec<u8>, bool)> {
        let mut source = self
            .session
            .open(remote_file)
            .await
            .map_err(|error| crate::sftp::remote("cannot open", remote_file, error))?;
        // One byte more than asked for tells whether the file is longer.
        let mut bytes = Vec::new();
        (&mut source)
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| crate::sftp::remote("cannot read", remote_file, error))?;
        let longer = bytes.len() as u64 > limit;
        bytes.truncate(limit as usize);
        Ok((bytes, longer))
    }

    /// The size of a remote file.
    pub async fn size(&self, remote_file: &str) -> Result<u64> {
        self.session
            .metadata(remote_file)
            .await
            .map(|metadata| metadata.size.unwrap_or(0))
            .map_err(|error| crate::sftp::remote("cannot look at", remote_file, error))
    }

    /// Copies a remote file to a local one, reporting progress.
    ///
    /// With `resume`, a local file that is shorter than the remote one is taken
    /// as the start of it and only the rest is fetched. Otherwise the local file is replaced.
    pub async fn download_tracked(
        &self,
        remote_file: &str,
        local_file: &Path,
        resume: bool,
        mut progress: impl FnMut(Progress) + Send,
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
                .map_err(|error| crate::sftp::local(folder, error))?;
        }
        let mut source = self
            .session
            .open(remote_file)
            .await
            .map_err(|error| crate::sftp::remote("cannot open", remote_file, error))?;
        let mut options = tokio::fs::OpenOptions::new();
        if offset > 0 {
            source
                .seek(SeekFrom::Start(offset))
                .await
                .map_err(|error| crate::sftp::remote("cannot continue", remote_file, error))?;
            options.append(true);
        } else {
            options.write(true).create(true).truncate(true);
        }
        let mut target = options
            .open(local_file)
            .await
            .map_err(|error| crate::sftp::local(local_file, error))?;

        let done = copy(&mut source, &mut target, offset, total, &mut progress)
            .await
            .map_err(|failure| match failure {
                Failure::Reading(error) => {
                    crate::sftp::remote("cannot download", remote_file, error)
                }
                Failure::Writing(error) => crate::sftp::local(local_file, error),
            })?;
        target
            .flush()
            .await
            .map_err(|error| crate::sftp::local(local_file, error))?;
        Ok(Transferred {
            bytes: done,
            resumed_from: offset,
        })
    }

    /// Copies a local file to a remote one, reporting progress.
    ///
    /// With `resume`, a remote file that is shorter than the local one is taken
    /// as the start of it and only the rest is sent. Otherwise the remote file is replaced.
    pub async fn upload_tracked(
        &self,
        local_file: &Path,
        remote_file: &str,
        resume: bool,
        mut progress: impl FnMut(Progress) + Send,
    ) -> Result<Transferred> {
        let mut source = tokio::fs::File::open(local_file)
            .await
            .map_err(|error| crate::sftp::local(local_file, error))?;
        let total = source
            .metadata()
            .await
            .map_err(|error| crate::sftp::local(local_file, error))?
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
                .map_err(|error| crate::sftp::remote("cannot open", remote_file, error))?;
            file.seek(SeekFrom::Start(offset))
                .await
                .map_err(|error| crate::sftp::remote("cannot continue", remote_file, error))?;
            file
        } else {
            self.session
                .create(remote_file)
                .await
                .map_err(|error| crate::sftp::remote("cannot create", remote_file, error))?
        };
        if offset > 0 {
            source
                .seek(SeekFrom::Start(offset))
                .await
                .map_err(|error| crate::sftp::local(local_file, error))?;
        }

        let done = copy(&mut source, &mut target, offset, total, &mut progress)
            .await
            .map_err(|failure| match failure {
                Failure::Reading(error) => crate::sftp::local(local_file, error),
                Failure::Writing(error) => crate::sftp::remote("cannot upload", remote_file, error),
            })?;
        // Without this the last block can still be on its way when the caller lists the folder.
        target
            .shutdown()
            .await
            .map_err(|error| crate::sftp::remote("cannot finish", remote_file, error))?;
        Ok(Transferred {
            bytes: done,
            resumed_from: offset,
        })
    }
}

/// Which end of a copy failed.
enum Failure {
    Reading(std::io::Error),
    Writing(std::io::Error),
}

/// Copies everything from `source` to `target`, calling `progress` after each chunk.
/// Returns the size of the file when done: `start` plus what was copied.
async fn copy(
    source: &mut (impl AsyncReadExt + Unpin),
    target: &mut (impl AsyncWriteExt + Unpin),
    start: u64,
    total: u64,
    progress: &mut (impl FnMut(Progress) + Send),
) -> std::result::Result<u64, Failure> {
    let mut buffer = vec![0u8; CHUNK];
    let mut done = start;
    progress(Progress { done, total });
    loop {
        let read = source.read(&mut buffer).await.map_err(Failure::Reading)?;
        if read == 0 {
            return Ok(done);
        }
        target
            .write_all(&buffer[..read])
            .await
            .map_err(Failure::Writing)?;
        done += read as u64;
        progress(Progress {
            done,
            total: total.max(done),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn copy_reports_progress_up_to_the_whole_size() {
        let data = vec![7u8; CHUNK * 2 + 100];
        let mut source = Cursor::new(data.clone());
        let mut target = Vec::new();
        let mut seen = Vec::new();

        let done = copy(
            &mut source,
            &mut target,
            0,
            data.len() as u64,
            &mut |progress| seen.push(progress.done),
        )
        .await
        .ok()
        .unwrap();

        assert_eq!(done, data.len() as u64);
        assert_eq!(target, data);
        // Once at the start, then after each chunk, always growing.
        assert_eq!(seen.first(), Some(&0));
        assert_eq!(seen.last(), Some(&(data.len() as u64)));
        assert!(seen.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[tokio::test]
    async fn copy_counts_from_where_a_resumed_transfer_starts() {
        let mut source = Cursor::new(vec![1u8; 100]);
        let mut target = Vec::new();
        let mut first = None;

        let done = copy(&mut source, &mut target, 900, 1000, &mut |progress| {
            first.get_or_insert(progress);
        })
        .await
        .ok()
        .unwrap();

        assert_eq!(done, 1000);
        assert_eq!(
            first,
            Some(Progress {
                done: 900,
                total: 1000
            })
        );
    }

    #[tokio::test]
    async fn the_total_never_falls_behind_the_done_count() {
        // A server that did not say the size (0) still gives a total that grows with the file.
        let mut source = Cursor::new(vec![1u8; 10]);
        let mut target = Vec::new();
        let mut last = None;

        copy(&mut source, &mut target, 0, 0, &mut |progress| {
            last = Some(progress)
        })
        .await
        .ok()
        .unwrap();

        assert_eq!(
            last,
            Some(Progress {
                done: 10,
                total: 10
            })
        );
    }
}
