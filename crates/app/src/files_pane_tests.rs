//! Tests of `files_pane.rs`.

use super::*;

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
fn a_finished_transfer_shows_its_result_and_a_failed_one_its_reason() {
    let mut state = loaded();
    state.busy = true;

    let done = Message::Transferred(Ok(("Uploaded a.txt".to_string(), None)));
    let _ = update(&mut state, done);
    assert!(!state.busy);
    assert_eq!(
        state.status,
        Some(Status::Done("Uploaded a.txt".to_string()))
    );

    let _ = update(
        &mut state,
        Message::Transferred(Err("disk full".to_string())),
    );
    assert_eq!(state.status, Some(Status::Failed("disk full".to_string())));
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
