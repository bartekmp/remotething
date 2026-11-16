//! Receiving the files the server copied: where each one goes, and the
//! request-by-request download that fills them.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use ironrdp_cliprdr::is_windows_device_name;
use ironrdp_cliprdr::pdu::{
    ClipboardFileAttributes, FileContentsFlags, FileContentsRequest, FileDescriptor,
};

/// How many bytes are asked for at a time.
pub(crate) const CHUNK: u32 = 256 * 1024;

/// One entry of what the server copied, as it will be written here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Planned {
    /// Its position in the server's list, which requests name it by.
    pub(crate) index: i32,
    pub(crate) target: PathBuf,
    pub(crate) size: u64,
    pub(crate) is_folder: bool,
}

/// A name that stays in one folder: no way up, no separator, no Windows device name.
fn is_plain(part: &str) -> bool {
    let mut components = Path::new(part).components();
    let single = matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(name)), None) if name == part
    );
    single && !part.contains('\\') && !is_windows_device_name(part)
}

/// The path of a descriptor under `folder`, or why it is refused. The names come from
/// the server, so every part must be plain: one that leads out of `folder` is never followed.
fn target_of(folder: &Path, descriptor: &FileDescriptor) -> Result<PathBuf, String> {
    let mut target = folder.to_path_buf();
    let parents = descriptor.relative_path.as_deref().unwrap_or("");
    for part in parents.split('\\').filter(|part| !part.is_empty()) {
        if !is_plain(part) {
            return Err(unsafe_name(parents));
        }
        target.push(part);
    }
    if !is_plain(&descriptor.name) {
        return Err(unsafe_name(&descriptor.name));
    }
    target.push(&descriptor.name);
    Ok(target)
}

fn unsafe_name(name: &str) -> String {
    format!("the server offered a file with an unsafe name ({name}); nothing was received")
}

/// Works out where everything goes and checks that nothing would be replaced,
/// before a single byte is written. Returns the plan and the bytes to be received.
pub(crate) fn plan(
    descriptors: &[FileDescriptor],
    folder: &Path,
) -> Result<(Vec<Planned>, u64), String> {
    let mut planned = Vec::new();
    let mut total = 0u64;
    for (index, descriptor) in descriptors.iter().enumerate() {
        let target = target_of(folder, descriptor)?;
        let is_folder = descriptor
            .attributes
            .is_some_and(|attributes| attributes.contains(ClipboardFileAttributes::DIRECTORY));
        check_free(folder, &target, is_folder)?;
        let size = if is_folder {
            0
        } else {
            descriptor
                .file_size
                .ok_or_else(|| format!("the server did not say how big {} is", descriptor.name))?
        };
        total = total.saturating_add(size);
        planned.push(Planned {
            index: i32::try_from(index).map_err(|_| "too many files".to_string())?,
            target,
            size,
            is_folder,
        });
    }
    Ok((planned, total))
}

/// Refuses a target that exists, except a real folder for a folder, and any link on the way.
fn check_free(folder: &Path, target: &Path, is_folder: bool) -> Result<(), String> {
    let below = target.strip_prefix(folder).unwrap_or(target);
    let mut here = folder.to_path_buf();
    let count = below.components().count();
    for (position, part) in below.components().enumerate() {
        here.push(part);
        let Ok(metadata) = std::fs::symlink_metadata(&here) else {
            continue;
        };
        let last = position + 1 == count;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{} is a link; nothing was received",
                here.display()
            ));
        }
        if last && !(is_folder && metadata.is_dir()) {
            return Err(format!(
                "{} already exists; nothing was received",
                here.display()
            ));
        }
        if !last && !metadata.is_dir() {
            return Err(format!(
                "{} is not a folder; nothing was received",
                here.display()
            ));
        }
    }
    Ok(())
}

/// A download in progress: one file at a time, one request at a time.
#[derive(Debug)]
pub(crate) struct Download {
    planned: Vec<Planned>,
    next: usize,
    current: Option<Current>,
    /// The stream id of the request that has not been answered.
    pub(crate) awaiting: Option<u32>,
    /// Bytes received so far, of all files.
    pub(crate) done: u64,
    pub(crate) total: u64,
    /// Files and folders that are complete.
    pub(crate) finished: usize,
    clip_data_id: Option<u32>,
}

#[derive(Debug)]
struct Current {
    file: File,
    path: PathBuf,
    index: i32,
    size: u64,
    position: u64,
    /// The size of the range last asked for.
    asked: u32,
}

/// What the download wants next.
pub(crate) enum Next {
    /// Send this request, and wait for its answer.
    Request(FileContentsRequest),
    /// Everything is received.
    Done,
}

impl Download {
    pub(crate) fn new(planned: Vec<Planned>, total: u64, clip_data_id: Option<u32>) -> Self {
        Self {
            planned,
            next: 0,
            current: None,
            awaiting: None,
            done: 0,
            total,
            finished: 0,
            clip_data_id,
        }
    }

    /// The answer to the request that was waiting: the bytes, or `None` when the server failed.
    pub(crate) fn received(&mut self, data: Option<Vec<u8>>) -> Result<(), String> {
        self.awaiting = None;
        let (Some(current), Some(data)) = (self.current.as_mut(), data) else {
            return Err("the server could not read the file".to_string());
        };
        if data.is_empty() || data.len() as u64 > u64::from(current.asked) {
            return Err("the server sent a wrong amount of data".to_string());
        }
        current
            .file
            .write_all(&data)
            .map_err(|error| format!("{}: {error}", current.path.display()))?;
        current.position += data.len() as u64;
        self.done += data.len() as u64;
        Ok(())
    }

    /// Moves on: opens the next file or folder when the last is complete, and says
    /// what to ask for. Call again after each answer.
    pub(crate) fn next(&mut self, stream_id: u32) -> Result<Next, String> {
        loop {
            if let Some(current) = &self.current {
                if current.position >= current.size {
                    self.current = None;
                    self.finished += 1;
                    continue;
                }
                let wanted = (current.size - current.position).min(u64::from(CHUNK)) as u32;
                let request = FileContentsRequest {
                    stream_id,
                    index: current.index,
                    flags: FileContentsFlags::RANGE,
                    position: current.position,
                    requested_size: wanted,
                    data_id: self.clip_data_id,
                };
                if let Some(current) = self.current.as_mut() {
                    current.asked = wanted;
                }
                self.awaiting = Some(stream_id);
                return Ok(Next::Request(request));
            }
            let Some(planned) = self.planned.get(self.next) else {
                return Ok(Next::Done);
            };
            self.next += 1;
            if planned.is_folder {
                std::fs::create_dir_all(&planned.target)
                    .map_err(|error| format!("{}: {error}", planned.target.display()))?;
                self.finished += 1;
                continue;
            }
            // `create_new` never follows a link or replaces what is there.
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&planned.target)
                .map_err(|error| format!("{}: {error}", planned.target.display()))?;
            self.current = Some(Current {
                file,
                path: planned.target.clone(),
                index: planned.index,
                size: planned.size,
                position: 0,
                asked: 0,
            });
        }
    }

    /// Stops, removing the file that was only partly received.
    pub(crate) fn abandon(&mut self) {
        if let Some(current) = self.current.take() {
            drop(current.file);
            let _ = std::fs::remove_file(&current.path);
        }
    }
}

#[cfg(test)]
#[path = "files_receive_tests.rs"]
mod tests;
