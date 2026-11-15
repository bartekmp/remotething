//! Which optional features are switched on, and where their files go.
//!
//! Set from the preferences when the app starts and whenever they change;
//! read where a feature adds a button or does its work. The window's
//! update and view run on one thread, so the values live there; tests, which
//! each run on a thread of their own, do not disturb each other.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use crate::preferences::{self, Features, Preferences};

#[derive(Default)]
struct Switches {
    features: Features,
    recordings: PathBuf,
}

thread_local! {
    static SWITCHES: RefCell<Switches> = RefCell::new(Switches::default());
}

/// Takes the switches from `preferences`; recordings go beside `connections_file`.
pub(crate) fn apply(preferences: &Preferences, connections_file: &Path) {
    SWITCHES.with(|switches| {
        *switches.borrow_mut() = Switches {
            features: preferences.features,
            recordings: preferences::recordings_beside(connections_file),
        };
    });
}

/// Whether SSH terminals can be recorded.
pub(crate) fn console_recording() -> bool {
    SWITCHES.with(|switches| switches.borrow().features.console_recording)
}

/// Whether recordings also keep what was typed.
pub(crate) fn record_keystrokes() -> bool {
    SWITCHES.with(|switches| switches.borrow().features.record_keystrokes)
}

/// Whether every SSH session starts recording by itself.
pub(crate) fn record_every_session() -> bool {
    SWITCHES.with(|switches| {
        let features = switches.borrow().features;
        features.console_recording && features.record_every_session
    })
}

/// The folder recordings are written to.
pub(crate) fn recordings_folder() -> PathBuf {
    SWITCHES.with(|switches| switches.borrow().recordings.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_is_off_until_applied() {
        assert!(!console_recording());
        assert!(!record_every_session());
    }

    #[test]
    fn recording_every_session_needs_recording_itself_to_be_on() {
        let mut preferences = Preferences::default();
        preferences.features.record_every_session = true;
        apply(&preferences, Path::new("/x/connections.toml"));
        assert!(!record_every_session());

        preferences.features.console_recording = true;
        apply(&preferences, Path::new("/x/connections.toml"));
        assert!(record_every_session());
        assert_eq!(recordings_folder(), Path::new("/x/recordings"));
    }
}
