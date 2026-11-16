//! Tests of `files_menu.rs`.

use super::*;

fn pane() -> State {
    let mut state = State::empty();
    state.root = "/home/ops".to_string();
    state
}

#[test]
fn a_file_offers_preview_and_a_folder_offers_a_new_folder_inside() {
    assert!(actions(EntryKind::File).contains(&Action::Preview));
    assert!(!actions(EntryKind::File).contains(&Action::NewFolder));
    assert!(actions(EntryKind::Folder).contains(&Action::NewFolder));
    assert!(!actions(EntryKind::Folder).contains(&Action::Preview));
}

#[test]
fn a_special_file_only_offers_copying_its_path() {
    assert_eq!(actions(EntryKind::Other), [Action::CopyPath]);
}

#[test]
fn names_that_leave_the_folder_or_are_empty_are_not_valid() {
    for bad in ["", "  ", ".", "..", "a/b", "/etc", "a\0b"] {
        assert!(!is_valid_name(bad), "{bad:?}");
    }
    for good in ["notes.txt", "my file", ".hidden", "zażółć"] {
        assert!(is_valid_name(good), "{good:?}");
    }
}

#[test]
fn opening_the_menu_selects_the_row() {
    let mut state = pane();

    open(
        &mut state,
        "/home/ops/notes.txt".to_string(),
        EntryKind::File,
    );

    assert_eq!(
        state.selected,
        Some(("/home/ops/notes.txt".to_string(), EntryKind::File))
    );
    assert_eq!(state.menu.unwrap().mode, Mode::Actions);
}

#[test]
fn renaming_starts_with_the_current_name() {
    let mut state = pane();
    open(
        &mut state,
        "/home/ops/notes.txt".to_string(),
        EntryKind::File,
    );

    let _ = choose(&mut state, Action::Rename);

    assert_eq!(
        state.menu.unwrap().mode,
        Mode::Renaming("notes.txt".to_string())
    );
}

#[test]
fn typing_changes_the_name_in_a_name_mode_only() {
    let mut state = pane();
    open(&mut state, "/home/ops/logs".to_string(), EntryKind::Folder);
    typed(&mut state, "ignored".to_string());
    assert_eq!(state.menu.as_ref().unwrap().mode, Mode::Actions);

    let _ = choose(&mut state, Action::NewFolder);
    typed(&mut state, "archive".to_string());

    assert_eq!(
        state.menu.unwrap().mode,
        Mode::NewFolder("archive".to_string())
    );
}

#[test]
fn deleting_asks_first_and_does_nothing_until_confirmed() {
    let mut state = pane();
    open(
        &mut state,
        "/home/ops/notes.txt".to_string(),
        EntryKind::File,
    );

    let _ = choose(&mut state, Action::Delete);

    assert_eq!(state.menu.unwrap().mode, Mode::ConfirmDelete);
}

#[test]
fn copying_the_path_closes_the_menu() {
    let mut state = pane();
    open(
        &mut state,
        "/home/ops/notes.txt".to_string(),
        EntryKind::File,
    );

    let _ = choose(&mut state, Action::CopyPath);

    assert!(state.menu.is_none());
}

#[test]
fn an_invalid_name_is_not_submitted_and_the_panel_stays() {
    let mut state = pane();
    open(
        &mut state,
        "/home/ops/notes.txt".to_string(),
        EntryKind::File,
    );
    let _ = choose(&mut state, Action::Rename);
    typed(&mut state, "../escape".to_string());

    let _ = submit(&mut state);

    assert!(matches!(state.menu.unwrap().mode, Mode::Renaming(_)));
}

#[test]
fn keeping_the_same_name_closes_the_menu_without_asking_the_server() {
    let mut state = pane();
    open(
        &mut state,
        "/home/ops/notes.txt".to_string(),
        EntryKind::File,
    );
    let _ = choose(&mut state, Action::Rename);

    let _ = submit(&mut state);

    assert!(state.menu.is_none());
}

#[test]
fn a_failed_operation_shows_its_reason() {
    let mut state = pane();

    let _ = done(&mut state, Err("cannot delete /x: failure".to_string()));

    assert_eq!(
        state.status,
        Some(Status::Failed("cannot delete /x: failure".to_string()))
    );
}

#[test]
fn a_finished_operation_clears_the_selection_and_forgets_the_old_listing() {
    let mut state = pane();
    state.selected = Some(("/home/ops/a".to_string(), EntryKind::File));
    state.listings.insert(
        "/home/ops".to_string(),
        crate::files_pane::Listing::Loaded(vec![]),
    );

    let _ = done(&mut state, Ok("/home/ops".to_string()));

    assert_eq!(state.selected, None);
    assert!(!state.listings.contains_key("/home/ops"));
}
