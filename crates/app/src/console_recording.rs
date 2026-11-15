//! A recording of an SSH terminal in progress, written as asciicast.

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use remotething_capture::asciicast::Recorder;

use crate::capture_files::{create_file, create_private_folder, stamped_name, unix_now};

/// A recording that is being written to a file.
pub(crate) struct Recording {
    recorder: Recorder<BufWriter<File>>,
    started: Instant,
    path: PathBuf,
}

impl Recording {
    /// Starts a recording in `folder`, named after the connection and the time.
    pub(crate) fn start(
        folder: &Path,
        connection: &str,
        columns: u16,
        lines: u16,
        keystrokes: bool,
    ) -> io::Result<Self> {
        create_private_folder(folder)?;
        let unix = unix_now();
        let (file, path) = create_file(folder, &stamped_name(connection, unix, "cast"))?;
        let recorder = Recorder::start(BufWriter::new(file), columns, lines, unix, keystrokes)?;
        Ok(Self {
            recorder,
            started: Instant::now(),
            path,
        })
    }

    /// Where the recording is written.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// How long the recording has been going.
    pub(crate) fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// What the server sent.
    pub(crate) fn output(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.recorder.output(self.started.elapsed(), bytes)
    }

    /// What the user typed.
    pub(crate) fn input(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.recorder.input(self.started.elapsed(), bytes)
    }

    /// The terminal changed size.
    pub(crate) fn resize(&mut self, columns: u16, lines: u16) -> io::Result<()> {
        self.recorder.resize(self.started.elapsed(), columns, lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_recording_is_private_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let folder = directory.path().join("recordings");

        let recording = Recording::start(&folder, "web-01", 80, 24, false).unwrap();

        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&folder), 0o700);
        assert_eq!(mode(recording.path()), 0o600);
    }

    #[test]
    fn a_recording_is_a_valid_cast_file_that_keeps_what_was_written() {
        let directory = tempfile::tempdir().unwrap();
        let mut recording = Recording::start(directory.path(), "web-01", 80, 24, false).unwrap();

        recording.output(b"hello\r\n").unwrap();
        recording.resize(100, 30).unwrap();
        let path = recording.path().to_path_buf();
        drop(recording);

        let text = std::fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{text}");
        assert!(lines[0].contains("\"version\":2"));
        assert!(lines[1].contains("\"o\"") && lines[1].contains("hello"));
        assert!(lines[2].contains("100x30"));
    }
}
