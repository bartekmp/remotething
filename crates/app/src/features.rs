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
    desktop_recordings: PathBuf,
    console_recordings: PathBuf,
    screenshots: PathBuf,
}

thread_local! {
    static SWITCHES: RefCell<Switches> = RefCell::new(Switches::default());
}

/// Takes the switches and folders from `preferences`. A folder the user did not choose is
/// one beside `connections_file`.
pub(crate) fn apply(preferences: &Preferences, connections_file: &Path) {
    let chosen = &preferences.folders;
    let default_recordings = preferences::recordings_beside(connections_file);
    SWITCHES.with(|switches| {
        *switches.borrow_mut() = Switches {
            features: preferences.features,
            desktop_recordings: chosen
                .desktop_recordings
                .clone()
                .unwrap_or_else(|| default_recordings.clone()),
            console_recordings: chosen
                .console_recordings
                .clone()
                .unwrap_or(default_recordings),
            screenshots: chosen
                .screenshots
                .clone()
                .unwrap_or_else(|| preferences::screenshots_beside(connections_file)),
        };
    });
}

/// Whether screenshots can be taken.
pub(crate) fn screenshots() -> bool {
    SWITCHES.with(|switches| switches.borrow().features.screenshots)
}

/// The folder screenshots are written to.
pub(crate) fn screenshots_folder() -> PathBuf {
    SWITCHES.with(|switches| switches.borrow().screenshots.clone())
}

/// Whether SSH terminals can be recorded.
pub(crate) fn console_recording() -> bool {
    SWITCHES.with(|switches| switches.borrow().features.console_recording)
}

/// Whether remote desktops can be recorded.
pub(crate) fn desktop_recording() -> bool {
    SWITCHES.with(|switches| switches.borrow().features.desktop_recording)
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

/// The folder recordings of remote desktops are written to.
pub(crate) fn desktop_recordings_folder() -> PathBuf {
    SWITCHES.with(|switches| switches.borrow().desktop_recordings.clone())
}

/// The folder recordings of terminals are written to.
pub(crate) fn console_recordings_folder() -> PathBuf {
    SWITCHES.with(|switches| switches.borrow().console_recordings.clone())
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
        assert_eq!(desktop_recordings_folder(), Path::new("/x/recordings"));
        assert_eq!(console_recordings_folder(), Path::new("/x/recordings"));
        assert_eq!(screenshots_folder(), Path::new("/x/screenshots"));
    }

    #[test]
    fn a_chosen_folder_replaces_only_its_own_default() {
        let mut preferences = Preferences::default();
        preferences.folders.console_recordings = Some(PathBuf::from("/data/casts"));
        preferences.folders.screenshots = Some(PathBuf::from("/data/shots"));

        apply(&preferences, Path::new("/x/connections.toml"));

        assert_eq!(console_recordings_folder(), Path::new("/data/casts"));
        assert_eq!(screenshots_folder(), Path::new("/data/shots"));
        assert_eq!(desktop_recordings_folder(), Path::new("/x/recordings"));
    }
}
