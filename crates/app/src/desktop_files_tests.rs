//! Tests of `desktop_files.rs`.

use std::sync::Arc;

use remotething_core::{Protocol, Quality, ResolvedConnection, Store};
use remotething_secrets::{KnownHosts, MemoryStore, SecretStore};

use super::*;
use crate::desktop::Live;
use crate::session::Context;

struct Fixture {
    _directory: tempfile::TempDir,
    store: Store,
    secrets: Arc<dyn SecretStore>,
    known_hosts: KnownHosts,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let known_hosts = KnownHosts::load(&directory.path().join("known_hosts.toml")).unwrap();
        Self {
            _directory: directory,
            store: Store::default(),
            secrets: Arc::new(MemoryStore::new()),
            known_hosts,
        }
    }

    /// A connected desktop, and the end of its input queue.
    fn connected(&mut self) -> (State, tokio::sync::mpsc::Receiver<Input>) {
        let connection = ResolvedConnection {
            id: "win".to_string(),
            name: "win".to_string(),
            protocol: Protocol::Rdp,
            host: "win.example.net".to_string(),
            port: 3389,
            credential: None,
            jump: None,
            forwards: Vec::new(),
            desktop_quality: Quality::Balanced,
        };
        let context = Context {
            store: &self.store,
            secrets: &self.secrets,
            known_hosts: &mut self.known_hosts,
        };
        let (mut state, _task) = State::start(connection, None, &context);
        let input = state.connect_for_test();
        (state, input)
    }
}

fn live(state: &State) -> &Live {
    match &state.stage {
        Stage::Connected(live) => live,
        _ => panic!("not connected"),
    }
}

fn file(path: &str) -> RemoteFile {
    RemoteFile {
        path: path.to_string(),
        size: 10,
        is_folder: false,
    }
}

fn labels(state: &State) -> Vec<String> {
    live(state)
        .files
        .actions()
        .into_iter()
        .map(|(label, _)| label)
        .collect()
}

#[test]
fn sending_is_always_offered_and_receiving_only_once_the_server_copied_files() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = fixture.connected();
    assert_eq!(labels(&state).len(), 1);

    on_event(
        &mut state,
        FilesEvent::Available(vec![file("a.txt"), file("b.txt")]),
    );

    let labels = labels(&state);
    assert_eq!(labels.len(), 2);
    assert!(labels[0].contains('2'), "{labels:?}");
}

#[test]
fn a_copy_of_only_folders_has_nothing_to_receive() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = fixture.connected();
    let folder = RemoteFile {
        path: "docs".to_string(),
        size: 0,
        is_folder: true,
    };

    on_event(&mut state, FilesEvent::Available(vec![folder]));

    assert_eq!(labels(&state).len(), 1);
}

#[test]
fn while_receiving_the_strip_shows_progress_and_a_cancel_button_instead_of_receive() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = fixture.connected();
    on_event(&mut state, FilesEvent::Available(vec![file("a.txt")]));

    on_event(
        &mut state,
        FilesEvent::Progress {
            direction: Direction::Received,
            done: 1_000,
            total: 4_000,
        },
    );

    let status = live(&state).files.status().unwrap();
    assert!(
        status.contains("1.0 kB") && status.contains("4.0 kB"),
        "{status}"
    );
    assert!(labels(&state)[0].contains("Cancel"));
}

#[test]
fn a_finished_transfer_clears_the_progress_and_says_so() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = fixture.connected();
    on_event(
        &mut state,
        FilesEvent::Progress {
            direction: Direction::Sent,
            done: 1,
            total: 2,
        },
    );

    on_event(
        &mut state,
        FilesEvent::Finished {
            direction: Direction::Sent,
            files: 3,
        },
    );

    assert!(live(&state).files.status().is_none());
    assert!(
        live(&state)
            .note
            .as_ref()
            .and_then(Note::current)
            .unwrap()
            .contains('3')
    );
}

#[test]
fn a_failed_transfer_shows_why() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = fixture.connected();

    on_event(
        &mut state,
        FilesEvent::Failed {
            direction: Direction::Received,
            reason: "a.txt already exists".to_string(),
        },
    );

    let note = live(&state).note.as_ref().and_then(Note::current);
    assert_eq!(note, Some("a.txt already exists"));
}

#[test]
fn picked_files_and_a_picked_folder_are_passed_to_the_session() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = fixture.connected();

    let _ = update(
        &mut state,
        Message::Picked(Some(vec![PathBuf::from("/tmp/a")])),
    );
    let _ = update(
        &mut state,
        Message::FolderPicked(Some(PathBuf::from("/tmp/in"))),
    );
    let _ = update(&mut state, Message::CancelPressed);

    assert_eq!(
        input.try_recv().unwrap(),
        Input::OfferFiles(vec![PathBuf::from("/tmp/a")])
    );
    assert_eq!(
        input.try_recv().unwrap(),
        Input::ReceiveFiles(PathBuf::from("/tmp/in"))
    );
    assert_eq!(input.try_recv().unwrap(), Input::CancelFiles);
}

#[test]
fn cancelled_dialogs_send_nothing() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = fixture.connected();

    let _ = update(&mut state, Message::Picked(None));
    let _ = update(&mut state, Message::FolderPicked(None));

    assert!(input.try_recv().is_err());
}

#[tokio::test]
async fn files_dropped_one_after_another_are_offered_together() {
    let mut fixture = Fixture::new();
    let (mut state, mut input) = fixture.connected();

    let _ = update(&mut state, Message::Dropped(PathBuf::from("/tmp/a")));
    let _ = update(&mut state, Message::Dropped(PathBuf::from("/tmp/b")));
    assert!(
        input.try_recv().is_err(),
        "nothing goes before the drop settles"
    );
    let _ = update(&mut state, Message::DropSettled);

    let offered = Input::OfferFiles(vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")]);
    assert_eq!(input.try_recv().unwrap(), offered);
    assert!(input.try_recv().is_err());
}

#[test]
fn receiving_shows_a_bar_that_fills_and_sending_shows_none() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = fixture.connected();
    assert_eq!(live(&state).files.progress_share(), None);

    let progress = |direction, done| FilesEvent::Progress {
        direction,
        done,
        total: 400,
    };
    on_event(&mut state, progress(Direction::Received, 100));
    assert_eq!(live(&state).files.progress_share(), Some(0.25));

    on_event(&mut state, progress(Direction::Sent, 100));
    assert_eq!(live(&state).files.progress_share(), None);
}

#[test]
fn a_transfer_of_unknown_size_shows_no_bar() {
    let mut fixture = Fixture::new();
    let (mut state, _input) = fixture.connected();

    on_event(
        &mut state,
        FilesEvent::Progress {
            direction: Direction::Received,
            done: 10,
            total: 0,
        },
    );

    assert_eq!(live(&state).files.progress_share(), None);
}
