//! A recording of a remote desktop in progress, written as a Motion JPEG AVI.
//!
//! The picture is taken at a fixed rate. A thread encodes and writes, so the
//! window never waits for the encoder.

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use remotething_capture::avi::{self, Writer};
use remotething_capture::jpeg;

use crate::capture_files::{create_file, create_private_folder, stamped_name, unix_now};
use crate::features;
use crate::i18n::fl;
use crate::picture::Picture;

/// Pictures per second in the file.
const FRAMES_PER_SECOND: u32 = 10;
/// How often the window calls [`VideoRecording::tick`].
pub(crate) const TICK: Duration = Duration::from_millis(1000 / FRAMES_PER_SECOND as u64);

/// What the writing thread is asked to do, one job for every picture slot.
enum Job {
    /// A new picture.
    Picture(Arc<Vec<u8>>),
    /// The same picture as before.
    Repeat,
}

/// One file being written.
struct Output {
    jobs: Sender<Job>,
    thread: JoinHandle<io::Result<()>>,
    bytes: Arc<AtomicU64>,
    path: PathBuf,
    size: (u32, u32),
}

/// A recording that is being written to a file, or to a series of files.
pub(crate) struct VideoRecording {
    folder: PathBuf,
    connection: String,
    output: Option<Output>,
    started: Instant,
    /// How many picture slots have been handed to the thread, over all files.
    slots: u64,
    /// The picture that was last sent, to tell a changed one from an unchanged one.
    last_picture: Option<u64>,
    /// What went wrong in the thread, for the window to show.
    problem: Arc<Mutex<Option<String>>>,
}

impl VideoRecording {
    /// Starts recording `picture`'s size in `folder`.
    pub(crate) fn start(folder: &Path, connection: &str, picture: &Picture) -> io::Result<Self> {
        let problem = Arc::new(Mutex::new(None));
        let output = open_output(folder, connection, picture, &problem)?;
        Ok(Self {
            folder: folder.to_path_buf(),
            connection: connection.to_string(),
            output: Some(output),
            started: Instant::now(),
            slots: 0,
            last_picture: None,
            problem,
        })
    }

    /// Where the file being written is.
    pub(crate) fn path(&self) -> Option<&Path> {
        self.output.as_ref().map(|output| output.path.as_path())
    }

    /// How long the recording has been going.
    pub(crate) fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Takes the next slot of the recording: the picture if it changed, else a repeat.
    /// Returns what went wrong if the recording cannot go on.
    pub(crate) fn tick(&mut self, picture: &Picture) -> Result<(), String> {
        if let Some(reason) = self
            .problem
            .lock()
            .ok()
            .and_then(|mut problem| problem.take())
        {
            return Err(reason);
        }
        let Some(output) = &self.output else {
            return Err("the recording has ended".to_string());
        };
        // A new file when the desktop changed size or this one is big.
        let size = (picture.width, picture.height);
        if output.size != size || output.bytes.load(Ordering::Relaxed) >= avi::MAX_BYTES {
            self.finish_output()?;
            let next = open_output(&self.folder, &self.connection, picture, &self.problem)
                .map_err(|error| error.to_string())?;
            self.output = Some(next);
            // The first slot of a file is always a picture.
            self.last_picture = None;
        }
        let Some(output) = &self.output else {
            return Err("the recording has ended".to_string());
        };
        let job = if self.last_picture == Some(picture.serial()) {
            Job::Repeat
        } else {
            self.last_picture = Some(picture.serial());
            Job::Picture(picture.pixels())
        };
        output
            .jobs
            .send(job)
            .map_err(|_| "the writing thread stopped".to_string())?;
        self.slots += 1;
        Ok(())
    }

    /// Ends the recording. The file is complete when this returns.
    pub(crate) fn stop(mut self) -> Result<(), String> {
        self.finish_output()
    }

    fn finish_output(&mut self) -> Result<(), String> {
        let Some(output) = self.output.take() else {
            return Ok(());
        };
        // The thread finishes the file once there are no more jobs.
        drop(output.jobs);
        output
            .thread
            .join()
            .map_err(|_| "the writing thread failed".to_string())?
            .map_err(|error| error.to_string())
    }
}

/// The recording of one tab: there may be one, and there may be a reason the last one ended.
#[derive(Default)]
pub(crate) struct Video {
    recording: Option<VideoRecording>,
    problem: Option<String>,
}

impl Video {
    /// Starts recording, or stops the recording that is going. Does nothing at
    /// all while the feature is switched off.
    pub(crate) fn toggle(&mut self, connection: &str, picture: &Picture) {
        if let Some(recording) = self.recording.take() {
            self.problem = recording.stop().err();
            return;
        }
        if !features::desktop_recording() {
            return;
        }
        self.problem = None;
        match VideoRecording::start(&features::desktop_recordings_folder(), connection, picture) {
            Ok(recording) => self.recording = Some(recording),
            Err(error) => {
                self.problem = Some(format!("{} {error}", fl!("recording-failed")));
            }
        }
    }

    /// Takes the next slot of a recording that is going; one that fails ends and says why.
    pub(crate) fn tick(&mut self, picture: &Picture) {
        let Some(recording) = &mut self.recording else {
            return;
        };
        if let Err(reason) = recording.tick(picture) {
            self.recording = None;
            self.problem = Some(format!("{} {reason}", fl!("recording-stopped")));
        }
    }

    /// Whether a recording is going.
    pub(crate) fn is_active(&self) -> bool {
        self.recording.is_some()
    }

    /// How long the recording has been going.
    pub(crate) fn elapsed(&self) -> Option<Duration> {
        self.recording.as_ref().map(VideoRecording::elapsed)
    }

    /// The file being written.
    pub(crate) fn path(&self) -> Option<&Path> {
        self.recording.as_ref().and_then(VideoRecording::path)
    }

    /// Why the last recording ended by itself, or could not start.
    pub(crate) fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }
}

impl Drop for VideoRecording {
    /// Closing a tab in the middle of a recording still leaves a whole file.
    fn drop(&mut self) {
        let _ = self.finish_output();
    }
}

fn open_output(
    folder: &Path,
    connection: &str,
    picture: &Picture,
    problem: &Arc<Mutex<Option<String>>>,
) -> io::Result<Output> {
    create_private_folder(folder)?;
    let (file, path) = create_file(folder, &stamped_name(connection, unix_now(), "avi"))?;
    let size = (picture.width, picture.height);
    let writer = Writer::start(BufWriter::new(file), size.0, size.1, FRAMES_PER_SECOND)?;
    let (jobs, receiver) = mpsc::channel();
    let bytes = Arc::new(AtomicU64::new(0));
    let thread = std::thread::spawn({
        let bytes = Arc::clone(&bytes);
        let problem = Arc::clone(problem);
        move || {
            let result = write_all(writer, size, &receiver, &bytes);
            if let Err(error) = &result
                && let Ok(mut slot) = problem.lock()
            {
                *slot = Some(error.to_string());
            }
            result
        }
    });
    Ok(Output {
        jobs,
        thread,
        bytes,
        path,
        size,
    })
}

/// Encodes and writes every job until the sender is gone, then completes the file.
fn write_all(
    mut writer: Writer<BufWriter<File>>,
    size: (u32, u32),
    jobs: &mpsc::Receiver<Job>,
    bytes: &AtomicU64,
) -> io::Result<()> {
    for job in jobs {
        match job {
            Job::Picture(rgba) => {
                let encoded = jpeg::encode(size.0, size.1, &rgba)
                    .map_err(|reason| io::Error::new(io::ErrorKind::InvalidData, reason))?;
                writer.add_frame(&encoded)?;
            }
            Job::Repeat => writer.add_repeat()?,
        }
        bytes.store(writer.bytes(), Ordering::Relaxed);
    }
    writer.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(width: u32, height: u32, pixel: u8) -> Picture {
        Picture::new(width, height, vec![pixel; (width * height * 4) as usize])
    }

    fn files(folder: &Path) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        found.sort();
        found
    }

    #[test]
    fn a_recording_is_a_complete_avi_once_it_is_stopped() {
        let directory = tempfile::tempdir().unwrap();
        let first = picture(32, 32, 10);
        let mut recording = VideoRecording::start(directory.path(), "win 11", &first).unwrap();

        recording.tick(&first).unwrap();
        recording.tick(&first).unwrap(); // unchanged: a repeat
        recording.tick(&picture(32, 32, 200)).unwrap();
        let path = recording.path().unwrap().to_path_buf();
        recording.stop().unwrap();

        let bytes = std::fs::read(path).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"AVI ");
        // The header's frame count: three slots.
        assert_eq!(u32::from_le_bytes(bytes[48..52].try_into().unwrap()), 3);
    }

    #[test]
    fn dropping_a_recording_still_completes_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let first = picture(16, 16, 10);
        let mut recording = VideoRecording::start(directory.path(), "x", &first).unwrap();
        recording.tick(&first).unwrap();
        let path = recording.path().unwrap().to_path_buf();

        drop(recording);

        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.windows(4).any(|window| window == b"idx1"));
    }

    #[test]
    fn a_change_of_size_starts_a_new_file() {
        let directory = tempfile::tempdir().unwrap();
        let small = picture(16, 16, 10);
        let mut recording = VideoRecording::start(directory.path(), "x", &small).unwrap();
        recording.tick(&small).unwrap();

        recording.tick(&picture(32, 24, 10)).unwrap();
        recording.stop().unwrap();

        assert_eq!(files(directory.path()).len(), 2);
    }

    #[test]
    fn the_button_does_nothing_while_the_feature_is_off_and_records_when_it_is_on() {
        let directory = tempfile::tempdir().unwrap();
        let connections = directory.path().join("connections.toml");
        let frame = picture(16, 16, 10);
        let mut video = Video::default();

        features::apply(&crate::preferences::Preferences::default(), &connections);
        video.toggle("win", &frame);
        assert!(!video.is_active());

        let mut preferences = crate::preferences::Preferences::default();
        preferences.features.desktop_recording = true;
        features::apply(&preferences, &connections);
        video.toggle("win", &frame);
        assert!(video.is_active());
        video.tick(&frame);
        assert!(video.elapsed().is_some());
        let path = video.path().unwrap().to_path_buf();

        video.toggle("win", &frame);
        assert!(!video.is_active());
        assert!(video.problem().is_none());
        assert!(path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_for_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let first = picture(16, 16, 10);

        let recording = VideoRecording::start(directory.path(), "x", &first).unwrap();

        let mode = std::fs::metadata(recording.path().unwrap())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
