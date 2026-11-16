//! Copying files between the two machines over the clipboard channel.
//!
//! Sending: the user picks files, they are offered to the server, and the server
//! asks for the bytes when something is pasted there. Receiving: the server
//! announces files it copied, the user picks a folder, and the bytes are asked for.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

use ironrdp_cliprdr::pdu::{
    ClipboardFileAttributes, ClipboardFormatId, FileContentsFlags, FileContentsRequest,
    FileContentsResponse, FileDescriptor, MAX_FILE_COUNT,
};
use ironrdp_cliprdr::{Client, Cliprdr};

use crate::files_receive::{self, Download, Next};

type BoxedError = Box<dyn std::error::Error>;
type Messages = Vec<ironrdp_cliprdr::CliprdrSvcMessages<Client>>;

/// The most a single range of a file sent to the server may be.
const MAX_RANGE: u32 = 8 * 1024 * 1024;
/// Progress is reported at most this often.
const REPORT_EVERY: Duration = Duration::from_millis(100);
/// Seconds between the Windows epoch (1601) and the Unix epoch (1970).
const WINDOWS_EPOCH_GAP: u64 = 11_644_473_600;

/// One file or folder the server copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    /// Where it sits in what was copied, written with `/`.
    pub path: String,
    /// The size in bytes; 0 for a folder.
    pub size: u64,
    /// Whether this is a folder.
    pub is_folder: bool,
}

/// Which way files are moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// From the server to this machine.
    Received,
    /// From this machine to the server.
    Sent,
}

/// What happened to the files, for the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesEvent {
    /// The server copied files. They can be received into a folder.
    Available(Vec<RemoteFile>),
    /// Files are offered to the server and can now be pasted there.
    Offered {
        /// How many files, not counting folders.
        files: usize,
        /// Their size together.
        bytes: u64,
    },
    /// How far a transfer has come.
    Progress {
        /// Which way.
        direction: Direction,
        /// Bytes moved so far.
        done: u64,
        /// Bytes in all.
        total: u64,
    },
    /// A transfer is complete.
    Finished {
        /// Which way.
        direction: Direction,
        /// How many files and folders.
        files: usize,
    },
    /// A transfer stopped, or could not start.
    Failed {
        /// Which way.
        direction: Direction,
        /// Why.
        reason: String,
    },
}

/// What the channel noted about files since the last round.
#[derive(Debug, Default)]
pub(crate) struct FileNotes {
    /// The server copied files; this is the format to ask for their list with.
    pub(crate) remote_list: Option<ClipboardFormatId>,
    /// The list of what the server copied has arrived, with the id of its lock.
    pub(crate) list: Option<(Vec<FileDescriptor>, Option<u32>)>,
    /// The server wants bytes of the files we offered.
    pub(crate) requests: Vec<FileContentsRequest>,
    /// Answers to our requests: the stream, and the bytes or `None` for a failure.
    pub(crate) responses: Vec<(u32, Option<Vec<u8>>)>,
}

/// An entry of the files offered to the server, in the order of its list.
#[derive(Debug)]
struct Offered {
    /// `None` for a folder.
    path: Option<PathBuf>,
    size: u64,
}

/// The state of file transfers, both ways.
#[derive(Debug, Default)]
pub(crate) struct Files {
    /// Paths the user picked, not yet offered.
    to_offer: Option<Vec<PathBuf>>,
    offered: Vec<Offered>,
    offered_bytes: u64,
    sent: u64,
    /// The offered files have all been asked for, and the window has been told.
    sent_reported: bool,
    last_report: Option<Instant>,
    /// What the server copied last.
    remote: Option<(Vec<FileDescriptor>, Option<u32>)>,
    /// The folder the user wants the server's files in.
    to_receive: Option<PathBuf>,
    download: Option<Download>,
    cancel: bool,
    next_stream: u32,
}

impl Files {
    /// The user picked files to offer to the server.
    pub(crate) fn offer(&mut self, paths: Vec<PathBuf>) {
        self.to_offer = Some(paths);
    }

    /// The user chose a folder for the files the server copied.
    pub(crate) fn receive(&mut self, folder: PathBuf) {
        self.to_receive = Some(folder);
    }

    /// The user stopped the transfer that is going.
    pub(crate) fn cancel(&mut self) {
        self.cancel = true;
    }

    /// Does what the notes and the user's choices ask for. Messages for the server
    /// go into `messages`, what the window should hear into `events`.
    pub(crate) fn step(
        &mut self,
        channel: &mut Cliprdr<Client>,
        notes: FileNotes,
        messages: &mut Messages,
        events: &mut Vec<FilesEvent>,
    ) -> Result<(), BoxedError> {
        if let Some(format) = notes.remote_list {
            // A new copy replaces the old one; the list is asked for, and arrives below.
            self.remote = None;
            messages.push(channel.initiate_paste(format)?);
        }
        if let Some((list, clip_data_id)) = notes.list {
            events.push(FilesEvent::Available(summary(&list)));
            self.remote = Some((list, clip_data_id));
        }
        if let Some(paths) = self.to_offer.take() {
            self.offer_now(channel, &paths, messages, events)?;
        }
        for request in notes.requests {
            let response = self.answer(&request);
            messages.push(channel.submit_file_contents(response)?);
            self.report_sent(events);
        }
        self.finish_sent(events);
        if self.cancel {
            self.cancel = false;
            if let Some(mut download) = self.download.take() {
                download.abandon();
            }
        }
        if let Some(folder) = self.to_receive.take() {
            self.start_download(&folder, events);
        }
        self.drive_download(channel, notes.responses, messages, events)
    }

    fn offer_now(
        &mut self,
        channel: &mut Cliprdr<Client>,
        paths: &[PathBuf],
        messages: &mut Messages,
        events: &mut Vec<FilesEvent>,
    ) -> Result<(), BoxedError> {
        match collect(paths) {
            Ok((descriptors, offered)) => {
                self.offered_bytes = offered.iter().map(|entry| entry.size).sum();
                let files = offered.iter().filter(|entry| entry.path.is_some()).count();
                events.push(FilesEvent::Offered {
                    files,
                    bytes: self.offered_bytes,
                });
                self.offered = offered;
                self.sent = 0;
                self.sent_reported = false;
                messages.push(channel.initiate_file_copy(descriptors)?);
            }
            Err(reason) => events.push(FilesEvent::Failed {
                direction: Direction::Sent,
                reason,
            }),
        }
        Ok(())
    }

    /// The reply to the server asking for part of an offered file.
    fn answer(&mut self, request: &FileContentsRequest) -> FileContentsResponse<'static> {
        let stream = request.stream_id;
        let entry = usize::try_from(request.index)
            .ok()
            .and_then(|index| self.offered.get(index));
        let Some(entry) = entry else {
            return FileContentsResponse::new_error(stream);
        };
        if request.flags.contains(FileContentsFlags::SIZE) {
            return FileContentsResponse::new_size_response(stream, entry.size);
        }
        let (Some(path), true) = (&entry.path, request.requested_size <= MAX_RANGE) else {
            return FileContentsResponse::new_error(stream);
        };
        match read_range(path, request.position, request.requested_size) {
            Ok(bytes) => {
                self.sent += bytes.len() as u64;
                FileContentsResponse::new_data_response(stream, bytes)
            }
            Err(_) => FileContentsResponse::new_error(stream),
        }
    }

    fn report_sent(&mut self, events: &mut Vec<FilesEvent>) {
        if self.offered_bytes == 0 || !self.report_due(self.sent >= self.offered_bytes) {
            return;
        }
        events.push(FilesEvent::Progress {
            direction: Direction::Sent,
            done: self.sent.min(self.offered_bytes),
            total: self.offered_bytes,
        });
    }

    /// Tells the window once everything offered has been taken by the server.
    fn finish_sent(&mut self, events: &mut Vec<FilesEvent>) {
        if self.sent_reported || self.offered_bytes == 0 || self.sent < self.offered_bytes {
            return;
        }
        self.sent_reported = true;
        let files = self
            .offered
            .iter()
            .filter(|entry| entry.path.is_some())
            .count();
        events.push(FilesEvent::Finished {
            direction: Direction::Sent,
            files,
        });
    }

    /// Whether a progress report may go out now; `forced` for the last one.
    fn report_due(&mut self, forced: bool) -> bool {
        let due = forced
            || self
                .last_report
                .is_none_or(|at| at.elapsed() >= REPORT_EVERY);
        if due {
            self.last_report = Some(Instant::now());
        }
        due
    }

    fn start_download(&mut self, folder: &Path, events: &mut Vec<FilesEvent>) {
        let Some((descriptors, clip_data_id)) = &self.remote else {
            events.push(failed_receive("the server has no files copied".to_string()));
            return;
        };
        match files_receive::plan(descriptors, folder) {
            Ok((planned, total)) => {
                self.download = Some(Download::new(planned, total, *clip_data_id));
            }
            Err(reason) => events.push(failed_receive(reason)),
        }
    }

    /// Feeds the answers to the running download and asks for the next piece.
    fn drive_download(
        &mut self,
        channel: &mut Cliprdr<Client>,
        responses: Vec<(u32, Option<Vec<u8>>)>,
        messages: &mut Messages,
        events: &mut Vec<FilesEvent>,
    ) -> Result<(), BoxedError> {
        let Some(download) = self.download.as_mut() else {
            return Ok(());
        };
        let mut failure = None;
        for (stream, data) in responses {
            if download.awaiting == Some(stream) {
                failure = failure.or(download.received(data).err());
            }
        }
        let mut finished = false;
        if failure.is_none() && download.awaiting.is_none() {
            self.next_stream = self.next_stream.wrapping_add(1);
            match download.next(self.next_stream) {
                Ok(Next::Request(request)) => {
                    messages.push(channel.request_file_contents(request)?)
                }
                Ok(Next::Done) => finished = true,
                Err(reason) => failure = Some(reason),
            }
        }
        let (done, total, files) = (download.done, download.total, download.finished);
        if let Some(reason) = failure {
            download.abandon();
            self.download = None;
            events.push(failed_receive(reason));
        } else if finished {
            self.download = None;
            events.push(FilesEvent::Progress {
                direction: Direction::Received,
                done: total,
                total,
            });
            events.push(FilesEvent::Finished {
                direction: Direction::Received,
                files,
            });
        } else if self.report_due(false) {
            events.push(FilesEvent::Progress {
                direction: Direction::Received,
                done,
                total,
            });
        }
        Ok(())
    }
}

fn failed_receive(reason: String) -> FilesEvent {
    FilesEvent::Failed {
        direction: Direction::Received,
        reason,
    }
}

/// What the server copied, as the window shows it.
fn summary(descriptors: &[FileDescriptor]) -> Vec<RemoteFile> {
    descriptors
        .iter()
        .map(|descriptor| {
            let parents = descriptor.relative_path.as_deref().unwrap_or("");
            let path: Vec<&str> = parents
                .split('\\')
                .filter(|part| !part.is_empty())
                .chain([descriptor.name.as_str()])
                .collect();
            RemoteFile {
                path: path.join("/"),
                size: descriptor.file_size.unwrap_or(0),
                is_folder: descriptor
                    .attributes
                    .is_some_and(|a| a.contains(ClipboardFileAttributes::DIRECTORY)),
            }
        })
        .collect()
}

fn read_range(path: &Path, position: u64, size: u32) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(position))?;
    let mut bytes = Vec::new();
    file.take(u64::from(size)).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The descriptors for the picked paths, folders opened up, and the local file behind each.
fn collect(paths: &[PathBuf]) -> Result<(Vec<FileDescriptor>, Vec<Offered>), String> {
    let mut descriptors = Vec::new();
    let mut offered = Vec::new();
    for path in paths {
        add(path, &[], &mut descriptors, &mut offered)?;
    }
    if descriptors.is_empty() {
        return Err("there is nothing to send".to_string());
    }
    Ok((descriptors, offered))
}

fn add(
    path: &Path,
    parents: &[String],
    descriptors: &mut Vec<FileDescriptor>,
    offered: &mut Vec<Offered>,
) -> Result<(), String> {
    if descriptors.len() >= MAX_FILE_COUNT {
        return Err("too many files to send at once".to_string());
    }
    let metadata =
        std::fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    // Links and special files are not followed.
    if !metadata.is_file() && !metadata.is_dir() {
        return Ok(());
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} has no name", path.display()))?;
    if name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) {
        return Err(format!(
            "{name} has a character that Windows does not allow in names"
        ));
    }
    let mut descriptor = FileDescriptor::new(name.clone());
    if !parents.is_empty() {
        descriptor = descriptor.with_relative_path(parents.join("\\"));
    }
    let since_unix = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok());
    if let Some(since) = since_unix {
        let ticks = (since.as_secs() + WINDOWS_EPOCH_GAP) * 10_000_000;
        descriptor = descriptor.with_last_write_time(ticks);
    }
    if metadata.is_dir() {
        descriptors.push(descriptor.with_attributes(ClipboardFileAttributes::DIRECTORY));
        offered.push(Offered {
            path: None,
            size: 0,
        });
        let mut inside = parents.to_vec();
        inside.push(name);
        let mut children: Vec<_> = std::fs::read_dir(path)
            .map_err(|error| format!("{}: {error}", path.display()))?
            .filter_map(Result::ok)
            .map(|child| child.path())
            .collect();
        children.sort();
        for child in children {
            add(&child, &inside, descriptors, offered)?;
        }
    } else {
        let descriptor = descriptor
            .with_attributes(ClipboardFileAttributes::NORMAL)
            .with_file_size(metadata.len());
        descriptors.push(descriptor);
        offered.push(Offered {
            path: Some(path.to_path_buf()),
            size: metadata.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
