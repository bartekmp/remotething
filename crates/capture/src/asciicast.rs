//! The asciicast v2 format: a header line, then one line per event.
//!
//! Every line is valid on its own and each event is flushed as it is written,
//! so a recording that is cut off, by a crash or a dropped connection, still plays.

use std::io::{self, Write};
use std::time::Duration;

use serde_json::json;

use crate::utf8::Utf8Splitter;

/// Writes one recording.
pub struct Recorder<W: Write> {
    output: W,
    shown: Utf8Splitter,
    typed: Utf8Splitter,
    /// Whether what was typed is recorded too.
    keystrokes: bool,
}

impl<W: Write> Recorder<W> {
    /// Starts a recording of a terminal `columns` by `lines` big.
    ///
    /// `unix_time` is when it started, in seconds. What was typed is kept only
    /// when `keystrokes` is set, because passwords are typed there.
    pub fn start(
        mut output: W,
        columns: u16,
        lines: u16,
        unix_time: u64,
        keystrokes: bool,
    ) -> io::Result<Self> {
        let header = json!({
            "version": 2,
            "width": columns,
            "height": lines,
            "timestamp": unix_time,
            "env": { "TERM": "xterm-256color" },
        });
        writeln!(output, "{header}")?;
        output.flush()?;
        Ok(Self {
            output,
            shown: Utf8Splitter::new(),
            typed: Utf8Splitter::new(),
            keystrokes,
        })
    }

    /// What the server sent, `at` after the start.
    pub fn output(&mut self, at: Duration, bytes: &[u8]) -> io::Result<()> {
        let text = self.shown.push(bytes);
        self.event(at, "o", &text)
    }

    /// What the user typed, `at` after the start. Ignored unless keystrokes are recorded.
    pub fn input(&mut self, at: Duration, bytes: &[u8]) -> io::Result<()> {
        if !self.keystrokes {
            return Ok(());
        }
        let text = self.typed.push(bytes);
        self.event(at, "i", &text)
    }

    /// The terminal changed size, `at` after the start.
    pub fn resize(&mut self, at: Duration, columns: u16, lines: u16) -> io::Result<()> {
        self.event(at, "r", &format!("{columns}x{lines}"))
    }

    fn event(&mut self, at: Duration, kind: &str, text: &str) -> io::Result<()> {
        // Text that is still waiting for the rest of a character has nothing to show yet.
        if text.is_empty() {
            return Ok(());
        }
        // Seconds with microseconds, like asciinema writes them.
        let seconds = (at.as_secs_f64() * 1_000_000.0).round() / 1_000_000.0;
        let line = json!([seconds, kind, text]);
        writeln!(self.output, "{line}")?;
        self.output.flush()
    }

    /// Ends the recording and gives the output back.
    pub fn finish(self) -> W {
        self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(recorder: Recorder<Vec<u8>>) -> Vec<serde_json::Value> {
        let bytes = recorder.finish();
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).expect("every line is valid JSON"))
            .collect()
    }

    #[test]
    fn the_header_names_the_version_size_and_time() {
        let recorder = Recorder::start(Vec::new(), 80, 24, 1_700_000_000, false).unwrap();

        let written = lines(recorder);

        assert_eq!(written[0]["version"], 2);
        assert_eq!(written[0]["width"], 80);
        assert_eq!(written[0]["height"], 24);
        assert_eq!(written[0]["timestamp"], 1_700_000_000);
    }

    #[test]
    fn output_becomes_an_event_with_its_time() {
        let mut recorder = Recorder::start(Vec::new(), 80, 24, 0, false).unwrap();

        recorder
            .output(Duration::from_millis(1500), b"hello\r\n")
            .unwrap();

        let written = lines(recorder);
        assert_eq!(written[1], json!([1.5, "o", "hello\r\n"]));
    }

    #[test]
    fn control_characters_and_quotes_survive_as_valid_json() {
        let mut recorder = Recorder::start(Vec::new(), 80, 24, 0, false).unwrap();

        recorder
            .output(Duration::ZERO, b"\x1b[31m\"red\"\\\x07")
            .unwrap();

        let written = lines(recorder);
        assert_eq!(written[1][2], "\u{1b}[31m\"red\"\\\u{7}");
    }

    #[test]
    fn a_character_cut_between_two_outputs_is_written_whole() {
        let mut recorder = Recorder::start(Vec::new(), 80, 24, 0, false).unwrap();
        let bytes = "ż".as_bytes();

        recorder
            .output(Duration::from_secs(1), &bytes[..1])
            .unwrap();
        recorder
            .output(Duration::from_secs(2), &bytes[1..])
            .unwrap();

        let written = lines(recorder);
        // Nothing for the first half; the whole character with the second.
        assert_eq!(written.len(), 2);
        assert_eq!(written[1], json!([2.0, "o", "ż"]));
    }

    #[test]
    fn keystrokes_are_left_out_unless_asked_for() {
        let mut off = Recorder::start(Vec::new(), 80, 24, 0, false).unwrap();
        let mut on = Recorder::start(Vec::new(), 80, 24, 0, true).unwrap();

        off.input(Duration::ZERO, b"secret").unwrap();
        on.input(Duration::ZERO, b"ls").unwrap();

        assert_eq!(lines(off).len(), 1);
        assert_eq!(lines(on)[1], json!([0.0, "i", "ls"]));
    }

    #[test]
    fn a_resize_is_written_as_columns_and_lines() {
        let mut recorder = Recorder::start(Vec::new(), 80, 24, 0, false).unwrap();

        recorder.resize(Duration::from_secs(3), 120, 40).unwrap();

        assert_eq!(lines(recorder)[1], json!([3.0, "r", "120x40"]));
    }

    #[test]
    fn a_recording_cut_off_after_an_event_is_still_whole_lines() {
        let mut recorder = Recorder::start(Vec::new(), 80, 24, 0, false).unwrap();
        recorder.output(Duration::ZERO, b"a").unwrap();
        recorder.output(Duration::from_secs(1), b"b").unwrap();

        let bytes = recorder.finish();

        assert!(bytes.ends_with(b"\n"));
        assert_eq!(String::from_utf8(bytes).unwrap().lines().count(), 3);
    }
}
