//! FTP and FTPS (explicit TLS). The control connection is kept for listing and
//! other small commands; every transfer opens a connection of its own, so
//! several can run at the same time.

use std::io::SeekFrom;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use async_trait::async_trait;
use remotething_secrets::{KnownHosts, Secret};
use suppaftp::list::File as Listed;
use suppaftp::tokio::{AsyncRustlsConnector, AsyncRustlsFtpStream};
use suppaftp::tokio_rustls::TlsConnector;
use suppaftp::types::FileType;
use suppaftp::{FtpError, Status};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::entry::{Entry, EntryKind, sort};
use crate::fs::{OnProgress, RemoteFs};
use crate::ftp_tls::{Finding, pinned_config};
use crate::transfer::{Failure, Transferred, copy};
use crate::{Error, Result};

/// For reaching the server, the TLS handshake and the sign-in together.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// Where to connect and how to sign in.
#[derive(Clone)]
pub struct FtpSettings {
    /// A host name or an IP address.
    pub host: String,
    /// The TCP port, usually 21.
    pub port: u16,
    /// The user name; `anonymous` for public servers.
    pub username: String,
    /// The password.
    pub password: Secret,
    /// For FTPS: the trusted certificates. `None` is plain FTP, which sends the
    /// password and every file unprotected.
    pub secure: Option<KnownHosts>,
}

/// The files of an FTP or FTPS server.
pub struct FtpFs {
    settings: FtpSettings,
    /// The connection for listing and other small commands; opened again after it breaks.
    control: Mutex<Option<AsyncRustlsFtpStream>>,
}

impl FtpFs {
    /// Connects and signs in. The server's certificate, for FTPS, must already be
    /// trusted; otherwise this stops before the password is sent, with the
    /// certificate in the error.
    pub async fn connect(settings: FtpSettings) -> Result<Self> {
        let first = open(&settings).await?;
        Ok(Self {
            settings,
            control: Mutex::new(Some(first)),
        })
    }
}

/// Opens a connection, secures it when asked to and signs in.
async fn open(settings: &FtpSettings) -> Result<AsyncRustlsFtpStream> {
    match tokio::time::timeout(CONNECT_TIMEOUT, sign_in(settings)).await {
        Ok(result) => result,
        Err(_) => Err(Error::Unavailable(format!(
            "no answer from {} within {} seconds",
            settings.host,
            CONNECT_TIMEOUT.as_secs()
        ))),
    }
}

async fn sign_in(settings: &FtpSettings) -> Result<AsyncRustlsFtpStream> {
    let host = settings.host.as_str();
    let mut ftp = AsyncRustlsFtpStream::connect((host, settings.port))
        .await
        .map_err(unavailable)?;
    if let Some(known) = &settings.secure {
        let finding = Finding::default();
        let config = pinned_config(known.clone(), host, settings.port, Arc::clone(&finding));
        let connector = AsyncRustlsConnector::from(TlsConnector::from(Arc::new(config)));
        ftp = match ftp.into_secure(connector, host).await {
            Ok(secured) => secured,
            Err(error) => {
                let found = finding.lock().ok().and_then(|mut found| found.take());
                return Err(found.unwrap_or_else(|| unavailable(error)));
            }
        };
    }
    ftp.login(settings.username.as_str(), settings.password.expose())
        .await
        .map_err(|error| match error {
            FtpError::UnexpectedResponse(response) if response.status == Status::NotLoggedIn => {
                Error::LoginRefused
            }
            other => unavailable(other),
        })?;
    // Files are moved byte for byte; text mode would change line endings.
    ftp.transfer_type(FileType::Binary)
        .await
        .map_err(unavailable)?;
    Ok(ftp)
}

fn unavailable(error: impl std::fmt::Display) -> Error {
    Error::Unavailable(error.to_string())
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

/// Runs one command on the control connection, opening it again first if it broke.
/// A command that fails because the connection broke leaves it to be opened again next time.
macro_rules! on_control {
    ($self:expr, $action:expr, $path:expr, $ftp:ident => $command:expr) => {{
        let mut guard = $self.control.lock().await;
        if guard.is_none() {
            *guard = Some(open(&$self.settings).await?);
        }
        let Some($ftp) = guard.as_mut() else {
            return Err(unavailable("not connected"));
        };
        let result = $command.await;
        if matches!(result, Err(FtpError::ConnectionError(_))) {
            *guard = None;
        }
        result.map_err(|error| remote($action, $path, error))
    }};
}

#[async_trait]
impl RemoteFs for FtpFs {
    async fn home(&self) -> Result<String> {
        on_control!(self, "cannot find the home folder", ".", ftp => ftp.pwd())
    }

    async fn list(&self, folder: &str) -> Result<Vec<Entry>> {
        let lines = on_control!(self, "cannot list", folder, ftp => ftp.list(Some(folder)))?;
        let mut entries: Vec<Entry> = lines
            .iter()
            // Lines that are not entries, such as "total 12", do not parse and are skipped.
            .filter_map(|line| Listed::try_from(line.as_str()).ok())
            .map(|listed| entry_of(&listed))
            .filter(|entry| entry.name != "." && entry.name != "..")
            .collect();
        sort(&mut entries);
        Ok(entries)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let Some(folder) = crate::parent(path) else {
            // The root of the server.
            return Ok(true);
        };
        let name = path.rsplit('/').next().unwrap_or(path);
        let entries = self.list(&folder).await?;
        Ok(entries.iter().any(|entry| entry.name == name))
    }

    async fn size(&self, path: &str) -> Result<u64> {
        let size = on_control!(self, "cannot look at", path, ftp => ftp.size(path))?;
        Ok(size as u64)
    }

    async fn create_folder(&self, path: &str) -> Result<()> {
        on_control!(self, "cannot create", path, ftp => ftp.mkdir(path))
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
        // FTP servers differ in whether renaming onto a taken name replaces it; never do.
        if self.exists(to).await? {
            return Err(remote(
                "cannot rename",
                from,
                "the new name is already taken",
            ));
        }
        on_control!(self, "cannot rename", from, ftp => ftp.rename(from, to))
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        on_control!(self, "cannot delete", path, ftp => ftp.rm(path))
    }

    async fn remove_folder(&self, path: &str) -> Result<()> {
        on_control!(self, "cannot delete", path, ftp => ftp.rmdir(path))
    }

    async fn read_start(&self, path: &str, limit: u64) -> Result<(Vec<u8>, bool)> {
        let mut ftp = open(&self.settings).await?;
        let mut source = ftp
            .retr_as_stream(path)
            .await
            .map_err(|error| remote("cannot open", path, error))?;
        // One byte more than asked for tells whether the file is longer.
        let mut bytes = Vec::new();
        (&mut source)
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| remote("cannot read", path, error))?;
        // The rest of the file is not wanted: dropping the connection ends the transfer.
        drop(source);
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
        // Some servers cannot say a size; the progress then has no total.
        let total = self.size(remote_file).await.unwrap_or(0);
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

        let mut ftp = open(&self.settings).await?;
        if offset > 0 {
            ftp.resume_transfer(offset as usize)
                .await
                .map_err(|error| remote("cannot continue", remote_file, error))?;
        }
        let mut source = ftp
            .retr_as_stream(remote_file)
            .await
            .map_err(|error| remote("cannot open", remote_file, error))?;
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
        source
            .finish()
            .await
            .map_err(|error| remote("cannot download", remote_file, error))?;
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

        let mut ftp = open(&self.settings).await?;
        // Appending carries on after what is there; a plain store starts the file over.
        let mut target = if offset > 0 {
            ftp.append_with_stream(remote_file).await
        } else {
            ftp.put_with_stream(remote_file).await
        }
        .map_err(|error| remote("cannot create", remote_file, error))?;

        let done = copy(&mut source, &mut target, offset, total, progress)
            .await
            .map_err(|failure| match failure {
                Failure::Reading(error) => local(local_file, error),
                Failure::Writing(error) => remote("cannot upload", remote_file, error),
            })?;
        target
            .finish()
            .await
            .map_err(|error| remote("cannot finish", remote_file, error))?;
        Ok(Transferred {
            bytes: done,
            resumed_from: offset,
        })
    }
}

fn entry_of(listed: &Listed) -> Entry {
    let kind = if listed.is_directory() {
        EntryKind::Folder
    } else if listed.is_symlink() {
        EntryKind::Link
    } else if listed.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    };
    let modified = listed
        .modified()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since| since.as_secs());
    Entry {
        name: listed.name().to_string(),
        kind,
        size: Some(listed.size() as u64),
        modified,
        permissions: None,
    }
}
