//! A short message in the status strip that goes away by itself.

use std::time::{Duration, Instant};

/// How long a note stays.
const STAYS: Duration = Duration::from_secs(6);

/// A message and when it appeared.
#[derive(Debug, Clone)]
pub(crate) struct Note {
    text: String,
    since: Instant,
}

impl Note {
    pub(crate) fn new(text: String) -> Self {
        Self {
            text,
            since: Instant::now(),
        }
    }

    /// The text, while the note is still fresh.
    pub(crate) fn current(&self) -> Option<&str> {
        (self.since.elapsed() < STAYS).then_some(self.text.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_note_shows() {
        assert_eq!(Note::new("saved".to_string()).current(), Some("saved"));
    }

    #[test]
    fn an_old_note_is_gone() {
        let mut note = Note::new("saved".to_string());
        note.since = Instant::now() - STAYS;

        assert_eq!(note.current(), None);
    }
}
