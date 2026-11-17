//! Files of an SSH server that has no SFTP: shell commands for folders, and the SCP
//! protocol (`scp -f` and `scp -t` on the server) for the bytes of a file.
//!
//! The server needs a POSIX shell with `ls`, `test`, `mkdir`, `mv`, `rm`, `rmdir`, `wc`,
//! `head` and `scp`. A transfer always starts from the beginning: SCP cannot continue.

use std::path::Path;

use async_trait::async_trait;
use remotething_remote::ssh::{Connection, Output};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::entry::{Entry, EntryKind, sort};
use crate::fs::{OnProgress, RemoteFs};
use crate::transfer::{Failure, Transferred, copy};
use crate::{Error, Result};

/// The files of an SSH server, reached with commands run on a connection.
pub struct ScpFs {
    connection: Connection,
}

impl ScpFs {
    /// Uses `connection`; checks that the server can run commands.
    pub async fn open(connection: &Connection) -> Result<Self> {
        let files = Self {
            connection: connection.clone(),
        };
        files.home().await?;
        Ok(files)
    }

    /// Runs a command; fails with its complaint unless it ends with status 0.
    async fn run(&self, action: &'static str, path: &str, command: String) -> Result<Output> {
        let output = self
            .connection
            .exec(&command)
            .await
            .map_err(|error| remote(action, path, error))?;
        if output.succeeded() {
            Ok(output)
        } else {
            Err(remote(action, path, output.complaint()))
        }
    }
}

#[async_trait]
impl RemoteFs for ScpFs {
    async fn home(&self) -> Result<String> {
        let output = self
            .run("cannot find the home folder", ".", "pwd".to_string())
            .await?;
        Ok(text(&output.stdout).trim().to_string())
    }

    async fn list(&self, folder: &str) -> Result<Vec<Entry>> {
        // The slash makes `ls` look inside a link to a folder.
        let target = quote(&format!("{folder}/"));
        // GNU ls can give the time as seconds since 1970; others refuse the option and
        // are asked again without it, which leaves the entries without a time.
        let with_time = format!("LC_ALL=C ls -lAn --time-style=+%s -- {target}");
        let (output, date_fields) = match self.run("cannot list", folder, with_time).await {
            Ok(output) => (output, 1),
            Err(_) => {
                let plain = format!("LC_ALL=C ls -lAn -- {target}");
                (self.run("cannot list", folder, plain).await?, 3)
            }
        };
        let mut entries: Vec<Entry> = text(&output.stdout)
            .lines()
            .filter_map(|line| parse_ls(line, date_fields))
            .collect();
        sort(&mut entries);
        Ok(entries)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let output = self
            .connection
            .exec(&format!("test -e {}", quote(path)))
            .await
            .map_err(|error| remote("cannot look at", path, error))?;
        match output.status {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(remote("cannot look at", path, output.complaint())),
        }
    }

    async fn size(&self, path: &str) -> Result<u64> {
        let output = self
            .run("cannot look at", path, format!("wc -c < {}", quote(path)))
            .await?;
        text(&output.stdout)
            .trim()
            .parse()
            .map_err(|_| remote("cannot look at", path, "the size is not a number"))
    }

    async fn create_folder(&self, path: &str) -> Result<()> {
        self.run("cannot create", path, format!("mkdir -- {}", quote(path)))
            .await
            .map(|_| ())
    }

    async fn create_folders(&self, folder: &str) -> Result<()> {
        self.run(
            "cannot create",
            folder,
            format!("mkdir -p -- {}", quote(folder)),
        )
        .await
        .map(|_| ())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        let command = format!(
            "if [ -e {to} ]; then echo 'the new name is taken' >&2; exit 1; fi; mv -- {from} {to}",
            from = quote(from),
            to = quote(to),
        );
        self.run("cannot rename", from, command).await.map(|_| ())
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        self.run("cannot delete", path, format!("rm -- {}", quote(path)))
            .await
            .map(|_| ())
    }

    async fn remove_folder(&self, path: &str) -> Result<()> {
        self.run("cannot delete", path, format!("rmdir -- {}", quote(path)))
            .await
            .map(|_| ())
    }

    async fn read_start(&self, path: &str, limit: u64) -> Result<(Vec<u8>, bool)> {
        // One byte more than asked for tells whether the file is longer.
        let command = format!("head -c {} -- {}", limit + 1, quote(path));
        let mut bytes = self.run("cannot read", path, command).await?.stdout;
        let longer = bytes.len() as u64 > limit;
        bytes.truncate(limit as usize);
        Ok((bytes, longer))
    }

    async fn download_tracked(
        &self,
        remote_file: &str,
        local_file: &Path,
        _resume: bool,
        progress: OnProgress<'_>,
    ) -> Result<Transferred> {
        if let Some(folder) = local_file.parent() {
            tokio::fs::create_dir_all(folder)
                .await
                .map_err(|error| local(folder, error))?;
        }
        let mut stream = self
            .connection
            .exec_stream(&format!("scp -f -- {}", quote(remote_file)))
            .await
            .map_err(|error| remote("cannot download", remote_file, error))?;
        let failed = |reason: String| remote("cannot download", remote_file, reason);

        stream
            .write_all(&[0])
            .await
            .map_err(|e| failed(e.to_string()))?;
        let header = read_header(&mut stream).await.map_err(&failed)?;
        stream
            .write_all(&[0])
            .await
            .map_err(|e| failed(e.to_string()))?;

        let mut target = tokio::fs::File::create(local_file)
            .await
            .map_err(|error| local(local_file, error))?;
        let mut body = (&mut stream).take(header.size);
        let done = copy(&mut body, &mut target, 0, header.size, progress)
            .await
            .map_err(|failure| match failure {
                Failure::Reading(error) => failed(error.to_string()),
                Failure::Writing(error) => local(local_file, error),
            })?;
        target
            .flush()
            .await
            .map_err(|error| local(local_file, error))?;
        if done < header.size {
            return Err(failed("the connection ended early".to_string()));
        }
        read_ack(&mut stream).await.map_err(&failed)?;
        // A server that has nothing more may already have closed; the file is complete.
        let _ = stream.write_all(&[0]).await;
        Ok(Transferred {
            bytes: done,
            resumed_from: 0,
        })
    }

    async fn upload_tracked(
        &self,
        local_file: &Path,
        remote_file: &str,
        _resume: bool,
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
        if let Some(folder) = crate::parent(remote_file) {
            self.create_folders(&folder).await?;
        }
        let mut stream = self
            .connection
            .exec_stream(&format!("scp -t -- {}", quote(remote_file)))
            .await
            .map_err(|error| remote("cannot upload", remote_file, error))?;
        let failed = |reason: String| remote("cannot upload", remote_file, reason);

        read_ack(&mut stream).await.map_err(&failed)?;
        let name = remote_file.rsplit('/').next().unwrap_or(remote_file);
        // The name is only a label: scp -t stores under the path it was started with.
        let header = format!("C0644 {total} {}\n", name.replace('\n', " "));
        stream
            .write_all(header.as_bytes())
            .await
            .map_err(|e| failed(e.to_string()))?;
        read_ack(&mut stream).await.map_err(&failed)?;

        let done = copy(&mut source, &mut stream, 0, total, progress)
            .await
            .map_err(|failure| match failure {
                Failure::Reading(error) => local(local_file, error),
                Failure::Writing(error) => failed(error.to_string()),
            })?;
        stream
            .write_all(&[0])
            .await
            .map_err(|e| failed(e.to_string()))?;
        read_ack(&mut stream).await.map_err(&failed)?;
        stream.shutdown().await.map_err(|e| failed(e.to_string()))?;
        Ok(Transferred {
            bytes: done,
            resumed_from: 0,
        })
    }
}

/// What a source announces before the bytes of a file.
#[derive(Debug, PartialEq, Eq)]
struct Header {
    size: u64,
}

/// Reads the line that announces a file.
async fn read_header(
    stream: &mut (impl AsyncReadExt + Unpin),
) -> std::result::Result<Header, String> {
    parse_header(&read_line(stream).await?)
}

/// One line of the protocol, without its newline.
async fn read_line(
    stream: &mut (impl AsyncReadExt + Unpin),
) -> std::result::Result<String, String> {
    let mut line = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        let read = stream.read(&mut byte).await.map_err(|e| e.to_string())?;
        if read == 0 {
            return Err("the connection ended early".to_string());
        }
        if byte[0] == b'\n' {
            return Ok(String::from_utf8_lossy(&line).into_owned());
        }
        line.push(byte[0]);
    }
}

/// Reads the server's answer to what was sent: a zero byte, or a warning or error with a line.
async fn read_ack(stream: &mut (impl AsyncReadExt + Unpin)) -> std::result::Result<(), String> {
    let mut byte = [0u8; 1];
    let read = stream.read(&mut byte).await.map_err(|e| e.to_string())?;
    if read == 0 {
        return Err("the connection ended early".to_string());
    }
    match byte[0] {
        0 => Ok(()),
        _ => Err(read_line(stream).await?),
    }
}

/// Understands the line a source sends: `C0644 size name` announces a file.
/// A folder, an error message or anything else is an error.
fn parse_header(line: &str) -> std::result::Result<Header, String> {
    let Some(first) = line.chars().next() else {
        return Err("the server sent an empty line".to_string());
    };
    match first {
        'C' => {
            let size = line
                .split(' ')
                .nth(1)
                .and_then(|size| size.parse().ok())
                .ok_or_else(|| "the server sent a size that is not a number".to_string())?;
            Ok(Header { size })
        }
        'D' => Err("it is a folder".to_string()),
        // Warning (1) and error (2) arrive as a byte, then the message.
        '\u{1}' | '\u{2}' => Err(line[1..].to_string()),
        _ => Err(format!("the server sent {line}")),
    }
}

/// One line of `ls -lAn`, or nothing for the "total" line, devices and what is not understood.
/// `date_fields` is 1 when the time is seconds since 1970 (`--time-style=+%s`), and 3 for
/// the usual "Jun 8 2024" or "Jun 8 12:24", which is not read.
fn parse_ls(line: &str, date_fields: usize) -> Option<Entry> {
    let mode = line.get(..10)?;
    let kind = match mode.chars().next()? {
        '-' => EntryKind::File,
        'd' => EntryKind::Folder,
        'l' => EntryKind::Link,
        // Devices show two numbers where the size is, which would shift the name.
        'c' | 'b' => return None,
        'p' | 's' => EntryKind::Other,
        _ => return None,
    };
    // Mode, links, owner, group, size, the date, then the name.
    let mut rest = line;
    let mut fields = Vec::new();
    for _ in 0..5 + date_fields {
        rest = rest.trim_start();
        let end = rest.find(char::is_whitespace)?;
        fields.push(&rest[..end]);
        rest = &rest[end..];
    }
    let name = rest.strip_prefix(' ')?;
    let name = match kind {
        EntryKind::Link => name.split(" -> ").next().unwrap_or(name),
        _ => name,
    };
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let modified = if date_fields == 1 {
        fields[5].parse::<u64>().ok()
    } else {
        None
    };
    Some(Entry {
        name: name.to_string(),
        kind,
        size: fields[4].parse().ok(),
        modified,
        permissions: Some(permissions(mode)),
    })
}

/// The permission bits written as `rwxr-xr-x`.
fn permissions(mode: &str) -> u32 {
    let mut bits = 0;
    for (index, letter) in mode.chars().skip(1).take(9).enumerate() {
        let position = 8 - index as u32;
        if matches!(letter, 'r' | 'w' | 'x' | 's' | 't') {
            bits |= 1 << position;
        }
    }
    bits
}

/// `text` as one word for a POSIX shell.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
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

#[cfg(test)]
#[path = "scp_tests.rs"]
mod tests;
