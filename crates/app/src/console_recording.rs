//! A recording of an SSH terminal in progress, written as asciicast.

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use remotething_capture::asciicast::Recorder;

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
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        let (file, path) = create_file(folder, &file_name(connection, unix))?;
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

/// Creates `folder` and its parents. On Unix the folder is for its owner alone,
/// because a recording can hold anything that was on the screen.
fn create_private_folder(folder: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(folder)?;
    #[cfg(unix)]
    {
        // A folder that was there already keeps its old permissions unless they are set here.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Creates the file for a recording. If the name is taken (a second recording
/// in the same second), a number is put before the extension: `name-...-2.cast`.
fn create_file(folder: &Path, name: &str) -> io::Result<(File, PathBuf)> {
    let stem = name.strip_suffix(".cast").unwrap_or(name);
    for number in 1..=99 {
        let candidate = if number == 1 {
            name.to_string()
        } else {
            format!("{stem}-{number}.cast")
        };
        let path = folder.join(candidate);
        // `create_new` never overwrites a recording that is there.
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many recordings with this name",
    ))
}

/// `name-20261115-231500.cast`, in UTC, with the name made safe for a file system.
pub(crate) fn file_name(connection: &str, unix: u64) -> String {
    let safe: String = connection
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let safe = safe.trim_matches(|c| c == '-' || c == '.');
    let safe = if safe.is_empty() { "session" } else { safe };
    let (year, month, day) = civil_date(unix / 86_400);
    let seconds = unix % 86_400;
    format!(
        "{safe}-{year:04}{month:02}{day:02}-{:02}{:02}{:02}.cast",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// The date of a day counted from 1970-01-01, in the proleptic Gregorian calendar.
fn civil_date(days: u64) -> (u64, u64, u64) {
    // Howard Hinnant's algorithm, shifted so that years start in March.
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_is_the_first_of_january_1970() {
        assert_eq!(civil_date(0), (1970, 1, 1));
    }

    #[test]
    fn dates_around_a_leap_day_are_right() {
        // 2024-02-29 is day 19782 since the epoch.
        assert_eq!(civil_date(19_782), (2024, 2, 29));
        assert_eq!(civil_date(19_783), (2024, 3, 1));
    }

    #[test]
    fn the_name_has_the_connection_and_the_time() {
        // 2026-11-15 23:15:00 UTC.
        let unix = 1_794_784_500;

        assert_eq!(file_name("web-01", unix), "web-01-20261115-231500.cast");
    }

    #[test]
    fn a_name_is_made_safe_for_a_file_system() {
        assert_eq!(
            file_name("prod/db: main", 0),
            "prod-db--main-19700101-000000.cast"
        );
        assert_eq!(file_name("///", 0), "session-19700101-000000.cast");
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

    #[cfg(unix)]
    #[test]
    fn the_folder_and_the_files_are_for_their_owner_alone() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let folder = directory.path().join("recordings");
        // A folder that already exists with open permissions is closed again.
        std::fs::create_dir(&folder).unwrap();
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();

        let recording = Recording::start(&folder, "web-01", 80, 24, false).unwrap();

        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&folder), 0o700);
        assert_eq!(mode(recording.path()), 0o600);
    }

    #[test]
    fn a_second_recording_in_the_same_second_gets_a_number_instead_of_overwriting() {
        let directory = tempfile::tempdir().unwrap();

        let first = create_file(directory.path(), "web-01-20260101-000000.cast").unwrap();
        let second = create_file(directory.path(), "web-01-20260101-000000.cast").unwrap();
        let third = create_file(directory.path(), "web-01-20260101-000000.cast").unwrap();

        assert!(first.1.ends_with("web-01-20260101-000000.cast"));
        assert!(second.1.ends_with("web-01-20260101-000000-2.cast"));
        assert!(third.1.ends_with("web-01-20260101-000000-3.cast"));
    }
}
