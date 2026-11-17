//! Tests of `files_dual.rs`.

use super::*;
use crate::transfer_queue::Direction;

fn local_pane(root: &str) -> State {
    let mut pane = State::empty();
    pane.root = root.to_string();
    pane.is_local = true;
    pane.files = Some(Arc::new(LocalFs));
    pane
}

/// A remote pane with a local pane beside it, both on the local disk for the test.
fn dual(local_root: &str) -> State {
    let mut state = State::empty();
    state.files = Some(Arc::new(LocalFs));
    state.root = "/srv".to_string();
    state.other = Some(Box::new(local_pane(local_root)));
    state
}

#[tokio::test]
async fn the_local_pane_is_shown_and_hidden_by_the_same_button() {
    let mut state = State::empty();

    let _ = toggle(&mut state);
    assert!(state.other.as_ref().is_some_and(|other| other.is_local));
    assert!(!state.other.as_ref().unwrap().closable);

    let _ = toggle(&mut state);
    assert!(state.other.is_none());
}

#[test]
fn nothing_can_be_copied_without_a_selection_on_the_sending_side() {
    let mut state = dual("/home/me");
    assert!(!can_copy_to_local(&state));
    assert!(!can_copy_to_remote(&state));

    state.selected = Some(("/srv/a.txt".to_string(), EntryKind::File));
    state.other.as_mut().unwrap().selected = Some(("/home/me/b.txt".to_string(), EntryKind::File));

    assert!(can_copy_to_local(&state));
    assert!(can_copy_to_remote(&state));
}

#[test]
fn a_link_or_a_missing_pane_is_not_copied() {
    let mut state = dual("/home/me");
    state.other.as_mut().unwrap().selected = Some(("/home/me/l".to_string(), EntryKind::Link));
    assert!(!can_copy_to_remote(&state));

    state.other = None;
    state.selected = Some(("/srv/a.txt".to_string(), EntryKind::File));
    assert!(!can_copy_to_local(&state));
}

#[tokio::test]
async fn a_remote_file_is_queued_for_the_folder_selected_in_the_local_pane() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().to_string_lossy().replace('\\', "/");
    let mut state = dual(&folder);
    state.selected = Some(("/srv/a.txt".to_string(), EntryKind::File));

    let _ = copy_to_local(&mut state);

    let job = &state.queue.jobs[0];
    assert_eq!(job.direction, Direction::Download);
    assert_eq!(job.remote, "/srv/a.txt");
    assert_eq!(job.local, directory.path().join("a.txt"));
}

#[tokio::test]
async fn a_selected_local_folder_is_where_the_copy_lands() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().to_string_lossy().replace('\\', "/");
    let mut state = dual("/elsewhere");
    state.selected = Some(("/srv/a.txt".to_string(), EntryKind::File));
    state.other.as_mut().unwrap().selected = Some((folder, EntryKind::Folder));

    let _ = copy_to_local(&mut state);

    assert_eq!(state.queue.jobs[0].local, directory.path().join("a.txt"));
}

#[test]
fn a_copy_never_replaces_a_local_file() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("a.txt"), "mine").unwrap();
    let folder = directory.path().to_string_lossy().replace('\\', "/");
    let mut state = dual(&folder);
    state.selected = Some(("/srv/a.txt".to_string(), EntryKind::File));

    let _ = copy_to_local(&mut state);

    assert!(state.queue.jobs.is_empty());
    assert!(matches!(&state.status, Some(Status::Failed(reason)) if reason.contains("a.txt")));
}

#[tokio::test]
async fn a_finished_download_marks_the_local_pane_for_reading_again() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().to_string_lossy().replace('\\', "/");
    let mut state = dual(&folder);
    state.selected = Some(("/srv/a.txt".to_string(), EntryKind::File));
    let _ = copy_to_local(&mut state);
    let id = state.queue.jobs[0].id;
    let done = remotething_files::Transferred {
        bytes: 1,
        resumed_from: 0,
    };

    let _ = crate::files_pane::update(&mut state, Message::TransferEnded(id, Ok(done)));

    // The last transfer is over, so the local pane has been asked to read its folder again.
    assert!(!state.downloaded);
    let other = state.other.as_ref().unwrap();
    assert_eq!(
        other.listings.get(&folder),
        Some(&crate::files_pane::Listing::Loading)
    );
}

#[tokio::test]
async fn a_server_name_that_is_not_plain_is_not_copied() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().to_string_lossy().replace('\\', "/");
    for (path, kind) in [
        ("/srv/..", EntryKind::File),
        ("/srv/..", EntryKind::Folder),
        ("/srv/", EntryKind::File),
    ] {
        let mut state = dual(&folder);
        state.selected = Some((path.to_string(), kind));

        let _ = copy_to_local(&mut state);

        assert!(state.queue.jobs.is_empty(), "{path:?} was queued");
        assert!(matches!(state.status, Some(Status::Failed(_))), "{path:?}");
    }
}
