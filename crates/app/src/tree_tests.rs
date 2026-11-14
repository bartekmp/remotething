//! Tests of `tree.rs`.

use super::*;

fn folder(id: &str) -> Selection {
    Selection::Folder(id.to_string())
}

fn connection(id: &str) -> Selection {
    Selection::Connection(id.to_string())
}

#[test]
fn toggling_a_folder_twice_expands_it_again() {
    let mut state = State::default();

    update(&mut state, Message::FolderToggled("prod".to_string()));
    assert!(state.collapsed.contains("prod"));

    update(&mut state, Message::FolderToggled("prod".to_string()));
    assert!(state.collapsed.is_empty());
}

#[test]
fn selecting_a_connection_reports_the_change_but_does_not_open() {
    let mut state = State::default();

    let event = update(
        &mut state,
        Message::ConnectionSelected("web-01".to_string()),
    );

    assert_eq!(event, Some(Event::SelectionChanged));
    assert_eq!(
        state.selected(),
        Some(&Selection::Connection("web-01".to_string()))
    );
}

#[test]
fn selecting_a_folder_does_not_fold_it() {
    let mut state = State::default();

    update(&mut state, Message::FolderSelected("prod".to_string()));

    assert_eq!(
        state.selected(),
        Some(&Selection::Folder("prod".to_string()))
    );
    assert!(state.collapsed.is_empty());
}

#[test]
fn opening_reports_the_connection() {
    let mut state = State::default();

    let event = update(&mut state, Message::ConnectionOpened("web-01".to_string()));

    assert_eq!(event, Some(Event::Open("web-01".to_string())));
}

#[test]
fn the_toolbar_buttons_are_passed_to_the_window() {
    let mut state = State::default();

    assert_eq!(
        update(&mut state, Message::NewFolderPressed),
        Some(Event::NewFolder)
    );
    assert_eq!(
        update(&mut state, Message::NewConnectionPressed),
        Some(Event::NewConnection)
    );
}

#[test]
fn a_right_click_asks_for_the_menu() {
    let mut state = State::default();

    let on_row = update(&mut state, Message::MenuRequested(Some(folder("prod"))));
    let on_space = update(&mut state, Message::MenuRequested(None));

    assert_eq!(on_row, Some(Event::Menu(Some(folder("prod")))));
    assert_eq!(on_space, Some(Event::Menu(None)));
}

#[test]
fn a_submitted_rename_reports_the_typed_name_once() {
    let mut state = State::default();
    state.start_rename(connection("web-01"), "web-01".to_string());

    update(&mut state, Message::RenameChanged("web-02".to_string()));
    let event = update(&mut state, Message::RenameSubmitted);

    assert_eq!(
        event,
        Some(Event::Renamed(connection("web-01"), "web-02".to_string()))
    );
    assert_eq!(update(&mut state, Message::RenameSubmitted), None);
}

#[test]
fn a_cancelled_rename_reports_nothing() {
    let mut state = State::default();
    state.start_rename(connection("web-01"), "web-01".to_string());

    state.cancel();

    assert_eq!(update(&mut state, Message::RenameSubmitted), None);
}

#[test]
fn a_row_released_over_another_row_is_moved_there() {
    let mut state = State::default();
    update(
        &mut state,
        Message::ConnectionSelected("web-01".to_string()),
    );
    update(&mut state, Message::RowEntered(connection("web-01")));

    // Entering the next row is reported before leaving the first.
    update(&mut state, Message::RowEntered(folder("prod")));
    update(&mut state, Message::RowExited(connection("web-01")));
    assert_eq!(state.carried(), Some(&connection("web-01")));

    assert_eq!(
        state.release(),
        Some(Event::Moved {
            source: connection("web-01"),
            target: DropTarget::Row(folder("prod")),
        })
    );
    assert_eq!(state.release(), None);
}

#[test]
fn a_click_without_leaving_the_row_moves_nothing() {
    let mut state = State::default();
    update(&mut state, Message::RowEntered(connection("web-01")));
    update(
        &mut state,
        Message::ConnectionSelected("web-01".to_string()),
    );

    assert_eq!(state.carried(), None);
    assert_eq!(state.release(), None);
}

#[test]
fn a_row_released_over_the_empty_space_goes_to_the_top_level() {
    let mut state = State::default();
    update(&mut state, Message::ListEntered);
    update(&mut state, Message::RowEntered(connection("web-01")));
    update(
        &mut state,
        Message::ConnectionSelected("web-01".to_string()),
    );
    update(&mut state, Message::RowExited(connection("web-01")));

    assert_eq!(
        state.release(),
        Some(Event::Moved {
            source: connection("web-01"),
            target: DropTarget::TopLevel,
        })
    );
}

#[test]
fn a_row_released_outside_the_list_stays_where_it_was() {
    let mut state = State::default();
    update(
        &mut state,
        Message::ConnectionSelected("web-01".to_string()),
    );
    update(&mut state, Message::ListExited);

    assert_eq!(state.release(), None);
}

#[test]
fn folding_a_folder_does_not_pick_anything_up() {
    let mut state = State::default();
    update(&mut state, Message::FolderSelected("prod".to_string()));
    update(&mut state, Message::RowEntered(folder("other")));

    update(&mut state, Message::FolderToggled("other".to_string()));

    assert_eq!(state.release(), None);
}
