//! SMB 2 and 3 shares, in pure Rust. The root of the server lists its shares as
//! folders: `/` is the list, `/share` is a share, `/share/docs/a.txt` a file in it.
//!
//! One connection serves everything: listings take it for a moment, and a transfer
//! only uses it to open its file, then streams on a handle of its own, so
//! several transfers run side by side.

use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use async_trait::async_trait;
use remotething_secrets::Secret;
use smb2::{ClientConfig, ErrorKind, SmbClient, Tree};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::entry::{Entry, EntryKind, sort};
use crate::fs::{OnProgress, RemoteFs};
use crate::transfer::{Progress, Transferred};
use crate::{Error, Result};

/// For reaching the server and signing in.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// How much of a file is read or written at a time when no stream does it for us.
const BLOCK: usize = 1024 * 1024;
/// Shares of these types are printers, devices and the pipe for administration.
const DISK_SHARE: u32 = 0;
const TYPE_MASK: u32 = 0x00FF_FFFF;

/// Where to connect and how to sign in.
#[derive(Clone)]
pub struct SmbSettings {
    /// A host name or an IP address.
    pub host: String,
    /// The TCP port, usually 445.
    pub port: u16,
    /// The user name.
    pub username: String,
    /// The password.
    pub password: Secret,
    /// The domain, when the account belongs to one.
    pub domain: Option<String>,
}

/// The connection and the shares connected to on it.
struct Inner {
    client: SmbClient,
    trees: HashMap<String, Tree>,
}

/// The files of an SMB server.
pub struct SmbFs {
    inner: Mutex<Inner>,
}

impl SmbFs {
    /// Connects and signs in.
    pub async fn connect(settings: SmbSettings) -> Result<Self> {
        let config = ClientConfig {
            addr: format!("{}:{}", settings.host, settings.port),
            timeout: CONNECT_TIMEOUT,
            username: settings.username,
            password: settings.password.expose().to_string(),
            domain: settings.domain.unwrap_or_default(),
            auto_reconnect: true,
            ..ClientConfig::default()
        };
        let client = SmbClient::connect(config).await.map_err(|error| {
            if matches!(
                error.kind(),
                ErrorKind::AuthRequired | ErrorKind::GuestSessionRefused
            ) {
                Error::LoginRefused
            } else {
                Error::Unavailable(error.to_string())
            }
        })?;
        Ok(Self {
            inner: Mutex::new(Inner {
                client,
                trees: HashMap::new(),
            }),
        })
    }
}

/// Splits `/share/dir/file` into the share and the path inside it (`dir/file`, empty for the
/// share's root). `None` for the root of the server, which holds the shares.
fn split(path: &str) -> Option<(&str, &str)> {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.split_once('/').unwrap_or((trimmed, "")))
}

fn failed(action: &'static str, path: &str, error: impl std::fmt::Display) -> Error {
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

/// Runs `$body` with the connection and the connected share named by `$path`, connecting the
/// share the first time. `$body` is an expression that evaluates to a future.
macro_rules! in_share {
    ($self:expr, $action:expr, $path:expr, $client:ident, $tree:ident, $rel:ident => $body:expr) => {{
        let Some((share, $rel)) = split($path) else {
            return Err(failed($action, $path, "that is the list of shares"));
        };
        let mut guard = $self.inner.lock().await;
        let Inner { client, trees } = &mut *guard;
        if !trees.contains_key(share) {
            let tree = client
                .connect_share(share)
                .await
                .map_err(|error| failed($action, $path, error))?;
            trees.insert(share.to_string(), tree);
        }
        let Some($tree) = trees.get_mut(share) else {
            return Err(failed($action, $path, "the share is not connected"));
        };
        let $client = client;
        $body.await.map_err(|error| failed($action, $path, error))
    }};
}

#[async_trait]
impl RemoteFs for SmbFs {
    async fn home(&self) -> Result<String> {
        Ok("/".to_string())
    }

    async fn list(&self, folder: &str) -> Result<Vec<Entry>> {
        let Some(_) = split(folder) else {
            let mut guard = self.inner.lock().await;
            let shares = guard
                .client
                .list_shares()
                .await
                .map_err(|error| failed("cannot list", folder, error))?;
            let mut entries: Vec<Entry> = shares
                .into_iter()
                .filter(|share| share.share_type & TYPE_MASK == DISK_SHARE)
                .map(|share| Entry {
                    name: share.name,
                    kind: EntryKind::Folder,
                    size: None,
                    modified: None,
                    permissions: None,
                })
                .collect();
            sort(&mut entries);
            return Ok(entries);
        };
        let listed = in_share!(self, "cannot list", folder, client, tree, rel =>
            client.list_directory(tree, rel))?;
        let mut entries: Vec<Entry> = listed
            .into_iter()
            .filter(|entry| entry.name != "." && entry.name != "..")
            .map(|entry| Entry {
                kind: if entry.is_link() {
                    EntryKind::Link
                } else if entry.is_directory {
                    EntryKind::Folder
                } else {
                    EntryKind::File
                },
                size: Some(entry.size),
                modified: entry
                    .modified
                    .to_system_time()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|since| since.as_secs()),
                permissions: None,
                name: entry.name,
            })
            .collect();
        sort(&mut entries);
        Ok(entries)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let Some((share, rel)) = split(path) else {
            return Ok(true);
        };
        if rel.is_empty() {
            let listed = self.list("/").await?;
            return Ok(listed.iter().any(|entry| entry.name == share));
        }
        let found = in_share!(self, "cannot look at", path, client, tree, rel =>
            async { Ok::<_, smb2::Error>(client.stat(tree, rel).await.map(|_| true)) });
        match found? {
            Ok(exists) => Ok(exists),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(failed("cannot look at", path, error)),
        }
    }

    async fn size(&self, path: &str) -> Result<u64> {
        let info = in_share!(self, "cannot look at", path, client, tree, rel =>
            client.stat(tree, rel))?;
        Ok(info.size)
    }

    async fn create_folder(&self, path: &str) -> Result<()> {
        in_share!(self, "cannot create", path, client, tree, rel =>
            client.create_directory(tree, rel))
    }

    async fn create_folders(&self, folder: &str) -> Result<()> {
        let mut path = String::new();
        for part in folder.split('/').filter(|part| !part.is_empty()) {
            path.push('/');
            path.push_str(part);
            // The first part is the share itself, which is there or is not.
            if path.matches('/').count() > 1 && !self.exists(&path).await? {
                self.create_folder(&path).await?;
            }
        }
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        let (Some((share, from_rel)), Some((target_share, to_rel))) = (split(from), split(to))
        else {
            return Err(failed("cannot rename", from, "shares cannot be renamed"));
        };
        if share != target_share {
            return Err(failed(
                "cannot rename",
                from,
                "it cannot move to another share",
            ));
        }
        // The server refuses a name that is taken, so nothing is replaced.
        in_share!(self, "cannot rename", from, client, tree, _unused =>
            client.rename(tree, from_rel, to_rel))
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        in_share!(self, "cannot delete", path, client, tree, rel =>
            client.delete_file(tree, rel))
    }

    async fn remove_folder(&self, path: &str) -> Result<()> {
        in_share!(self, "cannot delete", path, client, tree, rel =>
            client.delete_directory(tree, rel))
    }

    async fn read_start(&self, path: &str, limit: u64) -> Result<(Vec<u8>, bool)> {
        let mut download = in_share!(self, "cannot open", path, client, tree, rel =>
            client.open_file_download(tree, rel))?;
        let mut bytes = Vec::new();
        while (bytes.len() as u64) <= limit {
            match download.next_chunk().await {
                Some(Ok(chunk)) => bytes.extend(chunk),
                Some(Err(error)) => return Err(failed("cannot read", path, error)),
                None => break,
            }
        }
        // The rest of the file is not wanted: dropping the download stops it.
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
        let mut options = tokio::fs::OpenOptions::new();
        if offset > 0 {
            options.append(true);
        } else {
            options.write(true).create(true).truncate(true);
        }
        let mut target = options
            .open(local_file)
            .await
            .map_err(|error| local(local_file, error))?;

        let mut done = offset;
        progress(Progress { done, total });
        if offset == 0 {
            // From the start the stream keeps several reads on the wire.
            let mut download = in_share!(self, "cannot open", remote_file, client, tree, rel =>
                client.open_file_download(tree, rel))?;
            while let Some(chunk) = download.next_chunk().await {
                let chunk = chunk.map_err(|error| failed("cannot download", remote_file, error))?;
                target
                    .write_all(&chunk)
                    .await
                    .map_err(|error| local(local_file, error))?;
                done += chunk.len() as u64;
                progress(Progress { done, total });
            }
        } else {
            // Carrying on from the middle needs reads at a position.
            let reader = in_share!(self, "cannot open", remote_file, client, tree, rel =>
                client.open_file_reader(tree, rel))?;
            while done < total {
                let wanted = (total - done).min(BLOCK as u64);
                let block = reader
                    .read_at(done, wanted)
                    .await
                    .map_err(|error| failed("cannot download", remote_file, error))?;
                if block.is_empty() {
                    break;
                }
                target
                    .write_all(&block)
                    .await
                    .map_err(|error| local(local_file, error))?;
                done += block.len() as u64;
                progress(Progress { done, total });
            }
            let _ = reader.close().await;
        }
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
        if offset > 0 {
            source
                .seek(SeekFrom::Start(offset))
                .await
                .map_err(|error| local(local_file, error))?;
        }

        let mut writer = if offset > 0 {
            in_share!(self, "cannot create", remote_file, client, tree, rel =>
                client.create_file_writer_at(tree, rel, offset))?
        } else {
            in_share!(self, "cannot create", remote_file, client, tree, rel =>
                client.create_file_writer(tree, rel))?
        };
        let mut done = offset;
        progress(Progress { done, total });
        let mut buffer = vec![0u8; BLOCK];
        loop {
            let read = source
                .read(&mut buffer)
                .await
                .map_err(|error| local(local_file, error))?;
            if read == 0 {
                break;
            }
            writer
                .write_chunk(&buffer[..read])
                .await
                .map_err(|error| failed("cannot upload", remote_file, error))?;
            done += read as u64;
            progress(Progress {
                done,
                total: total.max(done),
            });
        }
        writer
            .finish()
            .await
            .map_err(|error| failed("cannot finish", remote_file, error))?;
        Ok(Transferred {
            bytes: done,
            resumed_from: offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_holds_the_shares_and_a_path_splits_into_share_and_rest() {
        assert_eq!(split("/"), None);
        assert_eq!(split(""), None);
        assert_eq!(split("/docs"), Some(("docs", "")));
        assert_eq!(split("/docs/"), Some(("docs", "")));
        assert_eq!(split("/docs/a/b.txt"), Some(("docs", "a/b.txt")));
    }
}
