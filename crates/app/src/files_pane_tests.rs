//! Tests of `files_pane.rs`.

use super::*;
use crate::transfer_queue::JobState;

fn entry(name: &str, kind: EntryKind) -> Entry {
    Entry {
        name: name.to_string(),
        kind,
        size: Some(10),
        modified: None,
        permissions: None,
    }
}

/// A pane whose root `/home/ops` holds a folder `logs` and a file `notes.txt`.
fn loaded() -> State {
    let mut state = State::empty();
    state.root = "/home/ops".to_string();
    let listing = vec![
        entry("logs", EntryKind::Folder),
        entry("notes.txt", EntryKind::File),
    ];
    let listed = Message::Listed("/home/ops".to_string(), Ok(listing));
    let _ = update(&mut state, listed);
    state
}

fn paths(state: &State) -> Vec<String> {
    state
        .rows()
        .into_iter()
        .map(|row| format!("{}{}", "  ".repeat(row.depth), row.path))
        .collect()
}

#[test]
fn the_root_listing_is_shown_flat() {
    assert_eq!(paths(&loaded()), ["/home/ops/logs", "/home/ops/notes.txt"]);
}

#[test]
fn an_unfolded_folder_shows_its_contents_once_they_arrive() {
    let mut state = loaded();

    let _ = update(
        &mut state,
        Message::FolderToggled("/home/ops/logs".to_string()),
    );
    assert_eq!(paths(&state).len(), 2);

    let inside = vec![entry("app.log", EntryKind::File)];
    let listed = Message::Listed("/home/ops/logs".to_string(), Ok(inside));
    let _ = update(&mut state, listed);

    assert_eq!(
        paths(&state),
        [
            "/home/ops/logs",
            "  /home/ops/logs/app.log",
            "/home/ops/notes.txt"
        ]
    );
}

#[test]
fn folding_a_folder_hides_its_contents_again() {
    let mut state = loaded();
    let toggled = || Message::FolderToggled("/home/ops/logs".to_string());
    let _ = update(&mut state, toggled());
    let inside = vec![entry("app.log", EntryKind::File)];
    let _ = update(
        &mut state,
        Message::Listed("/home/ops/logs".to_string(), Ok(inside)),
    );

    let _ = update(&mut state, toggled());

    assert_eq!(paths(&state).len(), 2);
}

#[test]
fn only_a_selected_file_can_be_downloaded() {
    let mut state = loaded();
    assert_eq!(state.selected_file(), None);

    let _ = update(
        &mut state,
        Message::FolderToggled("/home/ops/logs".to_string()),
    );
    assert_eq!(state.selected_file(), None);

    let file = Message::Selected("/home/ops/notes.txt".to_string(), EntryKind::File);
    let _ = update(&mut state, file);
    assert_eq!(state.selected_file(), Some("/home/ops/notes.txt"));
}

#[test]
fn an_upload_goes_into_the_selected_folder_or_beside_the_selected_file() {
    let mut state = loaded();
    assert_eq!(state.upload_folder(), "/home/ops");

    let _ = update(
        &mut state,
        Message::FolderToggled("/home/ops/logs".to_string()),
    );
    assert_eq!(state.upload_folder(), "/home/ops/logs");

    let file = Message::Selected("/home/ops/logs/app.log".to_string(), EntryKind::File);
    let _ = update(&mut state, file);
    assert_eq!(state.upload_folder(), "/home/ops/logs");
}

#[test]
fn a_folder_that_cannot_be_listed_keeps_its_reason() {
    let mut state = loaded();

    let failed = Message::Listed(
        "/home/ops".to_string(),
        Err("permission denied".to_string()),
    );
    let _ = update(&mut state, failed);

    assert!(state.rows().is_empty());
    assert_eq!(
        state.listings.get("/home/ops"),
        Some(&Listing::Failed("permission denied".to_string()))
    );
}

#[test]
fn the_close_button_asks_the_session_to_close_the_pane() {
    let mut state = loaded();

    let (_, event) = update(&mut state, Message::ClosePressed);

    assert_eq!(event, Some(Event::Close));
}

#[test]
fn sizes_are_shown_in_decimal_units() {
    assert_eq!(format_size(512), "512 B");
    assert_eq!(format_size(1_500), "1.5 kB");
    assert_eq!(format_size(20_000_000), "20.0 MB");
    assert_eq!(format_size(3_200_000_000), "3.2 GB");
}

#[test]
fn an_upload_to_a_name_that_is_free_starts_at_once() {
    let mut state = loaded();
    let local = PathBuf::from("/tmp/new.txt");

    let _ = update(&mut state, Message::UploadChecked(local, Ok(false)));

    // No files service in a test, so nothing starts, but nothing is asked either.
    assert_eq!(state.pending_replace, None);
    let job = &state.queue.jobs[0];
    assert_eq!(job.direction, Direction::Upload);
    assert_eq!(job.remote, "/home/ops/new.txt");
}

#[test]
fn a_download_of_the_selected_file_is_queued() {
    let mut state = loaded();
    let _ = update(
        &mut state,
        Message::Selected("/home/ops/notes.txt".to_string(), EntryKind::File),
    );

    let _ = update(
        &mut state,
        Message::DownloadPicked(Some(PathBuf::from("/tmp/notes.txt"))),
    );

    let job = &state.queue.jobs[0];
    assert_eq!(job.direction, Direction::Download);
    assert_eq!(job.remote, "/home/ops/notes.txt");
    assert_eq!(job.local, PathBuf::from("/tmp/notes.txt"));
}

#[test]
fn a_transfer_that_ends_shows_its_result_and_can_be_retried_when_it_failed() {
    let mut state = loaded();
    let _ = update(
        &mut state,
        Message::UploadChecked(PathBuf::from("/tmp/a.txt"), Ok(false)),
    );
    let id = state.queue.jobs[0].id;

    let _ = update(
        &mut state,
        Message::TransferEnded(id, Err("connection lost".to_string())),
    );
    assert_eq!(
        state.queue.job(id).unwrap().state,
        JobState::Failed("connection lost".to_string())
    );

    let _ = update(&mut state, Message::RetryPressed(id));
    let job = state.queue.job(id).unwrap();
    assert_eq!(job.state, JobState::Waiting);
    assert!(job.resume);
}

#[test]
fn clearing_removes_the_transfers_that_are_over() {
    let mut state = loaded();
    let _ = update(
        &mut state,
        Message::UploadChecked(PathBuf::from("/tmp/a.txt"), Ok(false)),
    );
    let id = state.queue.jobs[0].id;
    let _ = update(&mut state, Message::CancelPressed(id));

    let _ = update(&mut state, Message::ClearPressed);

    assert!(state.queue.jobs.is_empty());
}

#[test]
fn an_upload_over_an_existing_file_waits_for_a_yes_and_names_the_remote_path() {
    let mut state = loaded();
    let local = PathBuf::from("/tmp/notes.txt");

    let _ = update(&mut state, Message::UploadChecked(local.clone(), Ok(true)));

    assert_eq!(
        state.pending_replace,
        Some((local, "/home/ops/notes.txt".to_string()))
    );
}

#[test]
fn cancelling_the_question_keeps_the_remote_file_and_clears_the_question() {
    let mut state = loaded();
    let local = PathBuf::from("/tmp/notes.txt");
    let _ = update(&mut state, Message::UploadChecked(local, Ok(true)));

    let _ = update(&mut state, Message::ReplaceCancelled);

    assert_eq!(state.pending_replace, None);
    assert!(state.queue.jobs.is_empty());
}

#[test]
fn confirming_the_question_clears_it_and_goes_on_with_the_upload() {
    let mut state = loaded();
    let local = PathBuf::from("/tmp/notes.txt");
    let _ = update(&mut state, Message::UploadChecked(local, Ok(true)));

    let _ = update(&mut state, Message::ReplaceConfirmed);

    assert_eq!(state.pending_replace, None);
}

#[test]
fn the_target_of_an_upload_follows_the_selection() {
    let mut state = loaded();
    let local = PathBuf::from("/home/me/report.pdf");
    assert_eq!(
        state.upload_target(&local).as_deref(),
        Some("/home/ops/report.pdf")
    );

    let _ = update(
        &mut state,
        Message::FolderToggled("/home/ops/logs".to_string()),
    );

    assert_eq!(
        state.upload_target(&local).as_deref(),
        Some("/home/ops/logs/report.pdf")
    );
}

#[test]
fn a_failed_check_is_shown_and_nothing_is_uploaded() {
    let mut state = loaded();
    let local = PathBuf::from("/tmp/notes.txt");

    let failed = Message::UploadChecked(local, Err("permission denied".to_string()));
    let _ = update(&mut state, failed);

    assert_eq!(
        state.status,
        Some(Status::Failed("permission denied".to_string()))
    );
    assert_eq!(state.pending_replace, None);
}
