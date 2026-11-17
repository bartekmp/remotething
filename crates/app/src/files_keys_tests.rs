//! Tests of `files_keys.rs`.

use remotething_files::Entry;

use super::*;
use crate::files_menu::Mode;
use crate::files_pane::Listing;

fn entry(name: &str, kind: EntryKind) -> Entry {
    Entry {
        name: name.to_string(),
        kind,
        size: Some(10),
        modified: None,
        permissions: None,
    }
}

/// Root `/home/ops` with folder `logs` (holding `app.log`) and file `notes.txt`.
fn pane() -> State {
    let mut state = State::empty();
    state.files = Some(std::sync::Arc::new(remotething_files::LocalFs));
    state.root = "/home/ops".to_string();
    state.listings.insert(
        "/home/ops".to_string(),
        Listing::Loaded(vec![
            entry("logs", EntryKind::Folder),
            entry("notes.txt", EntryKind::File),
        ]),
    );
    state.listings.insert(
        "/home/ops/logs".to_string(),
        Listing::Loaded(vec![entry("app.log", EntryKind::File)]),
    );
    state
}

fn press(state: &mut State, named: Named) {
    let _ = on_key(state, &Key::Named(named), Modifiers::empty());
}

fn selected(state: &State) -> Option<&str> {
    state.selected.as_ref().map(|(path, _)| path.as_str())
}

#[test]
fn down_starts_at_the_top_and_stops_at_the_bottom() {
    let mut state = pane();

    press(&mut state, Named::ArrowDown);
    assert_eq!(selected(&state), Some("/home/ops/logs"));
    press(&mut state, Named::ArrowDown);
    press(&mut state, Named::ArrowDown);
    assert_eq!(selected(&state), Some("/home/ops/notes.txt"));
}

#[test]
fn up_starts_at_the_bottom_and_stops_at_the_top() {
    let mut state = pane();

    press(&mut state, Named::ArrowUp);
    assert_eq!(selected(&state), Some("/home/ops/notes.txt"));
    press(&mut state, Named::ArrowUp);
    press(&mut state, Named::ArrowUp);
    assert_eq!(selected(&state), Some("/home/ops/logs"));
}

#[test]
fn home_and_end_jump_to_the_ends() {
    let mut state = pane();

    press(&mut state, Named::End);
    assert_eq!(selected(&state), Some("/home/ops/notes.txt"));
    press(&mut state, Named::Home);
    assert_eq!(selected(&state), Some("/home/ops/logs"));
}

#[test]
fn right_unfolds_a_folder_then_moves_into_it_and_left_goes_back_out() {
    let mut state = pane();
    press(&mut state, Named::ArrowDown);

    press(&mut state, Named::ArrowRight);
    assert!(state.expanded.contains("/home/ops/logs"));
    press(&mut state, Named::ArrowRight);
    assert_eq!(selected(&state), Some("/home/ops/logs/app.log"));

    // From inside, left goes to the folder; from the folder, left folds it.
    press(&mut state, Named::ArrowLeft);
    assert_eq!(selected(&state), Some("/home/ops/logs"));
    press(&mut state, Named::ArrowLeft);
    assert!(!state.expanded.contains("/home/ops/logs"));
}

#[test]
fn enter_toggles_a_folder_and_previews_a_file() {
    let mut state = pane();
    press(&mut state, Named::ArrowDown);

    press(&mut state, Named::Enter);
    assert!(state.expanded.contains("/home/ops/logs"));
    press(&mut state, Named::Enter);
    assert!(!state.expanded.contains("/home/ops/logs"));

    press(&mut state, Named::End);
    press(&mut state, Named::Enter);
    // The preview is on its way: the pane has asked for the file.
    assert!(state.preview.is_some());
}

#[test]
fn backspace_goes_up_a_folder() {
    let mut state = pane();

    press(&mut state, Named::Backspace);

    assert_eq!(
        state.root,
        "/home/ops".rsplit_once('/').map(|(up, _)| up).unwrap()
    );
}

#[test]
fn f2_asks_for_a_new_name_and_delete_asks_before_deleting() {
    let mut state = pane();
    press(&mut state, Named::End);

    press(&mut state, Named::F2);
    assert!(matches!(
        state.menu.as_ref().map(|menu| &menu.mode),
        Some(Mode::Renaming(name)) if name == "notes.txt"
    ));
    press(&mut state, Named::Escape);
    assert!(state.menu.is_none());

    press(&mut state, Named::Delete);
    assert!(matches!(
        state.menu.as_ref().map(|menu| &menu.mode),
        Some(Mode::ConfirmDelete)
    ));
}

#[test]
fn with_a_menu_open_the_arrows_do_not_move_the_selection() {
    let mut state = pane();
    press(&mut state, Named::End);
    press(&mut state, Named::Delete);

    press(&mut state, Named::ArrowUp);

    assert_eq!(selected(&state), Some("/home/ops/notes.txt"));
}

#[test]
fn nothing_is_renamed_or_deleted_without_a_selection() {
    let mut state = pane();

    press(&mut state, Named::F2);
    press(&mut state, Named::Delete);

    assert!(state.menu.is_none());
}

#[test]
fn escape_closes_a_preview() {
    let mut state = pane();
    press(&mut state, Named::End);
    press(&mut state, Named::Enter);
    assert!(state.preview.is_some());

    press(&mut state, Named::Escape);

    assert!(state.preview.is_none());
}

#[test]
fn tab_gives_the_keyboard_to_the_local_pane_and_back() {
    let mut state = pane();
    let mut local = pane();
    local.is_local = true;
    state.other = Some(Box::new(local));

    press(&mut state, Named::Tab);
    press(&mut state, Named::ArrowDown);

    assert!(state.focus_other);
    assert_eq!(selected(&state), None);
    assert_eq!(
        selected(state.other.as_ref().unwrap()),
        Some("/home/ops/logs")
    );

    press(&mut state, Named::Tab);
    press(&mut state, Named::ArrowDown);
    assert_eq!(selected(&state), Some("/home/ops/logs"));
}

#[test]
fn tab_does_nothing_without_a_second_pane() {
    let mut state = pane();

    press(&mut state, Named::Tab);

    assert!(!state.focus_other);
}

#[test]
fn clicking_in_a_pane_gives_it_the_keyboard() {
    let mut state = pane();
    state.other = Some(Box::new(pane()));

    let click = Message::Selected("/home/ops/notes.txt".to_string(), EntryKind::File);
    let _ = files_pane::update(&mut state, files_dual::wrap(click.clone()));
    assert!(state.focus_other);
    let _ = files_pane::update(&mut state, click);
    assert!(!state.focus_other);
}

#[test]
fn keys_with_control_or_alt_are_left_for_the_app() {
    let mut state = pane();

    let _ = on_key(&mut state, &Key::Named(Named::ArrowDown), Modifiers::CTRL);

    assert_eq!(selected(&state), None);
}
