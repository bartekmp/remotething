//! Tests of `wsl_import.rs`.

use super::*;

fn distro(name: &str, address: Option<&str>) -> WslDistro {
    WslDistro {
        name: name.to_string(),
        address: address.map(str::to_string),
        user: Some("bartek".to_string()),
        ssh_answers: Some(true),
    }
}

fn found() -> State {
    let mut state = State::default();
    let distros = vec![
        distro("Ubuntu", Some("172.28.0.5")),
        distro("Debian", Some("172.28.0.9")),
        distro("docker-desktop", None),
    ];
    let _ = update(&mut state, Message::Found(Ok(distros)));
    state
}

#[test]
fn everything_importable_starts_ticked_and_the_rest_is_listed_as_skipped() {
    let state = found();

    assert_eq!(state.chosen.len(), 2);
    assert_eq!(state.plan.as_ref().unwrap().skipped.len(), 1);
    assert!(!state.loading);
}

#[test]
fn unticking_leaves_a_distribution_out() {
    let mut state = found();

    let _ = update(&mut state, Message::Toggled("Ubuntu".to_string(), false));
    let (_, event) = update(&mut state, Message::ImportPressed);

    match event {
        Some(Event::Apply(_, names)) => assert_eq!(names, ["Debian"]),
        other => panic!("expected an import of Debian, got {other:?}"),
    }
}

#[test]
fn nothing_is_applied_until_import_is_pressed() {
    let mut state = found();

    let (_, event) = update(&mut state, Message::Toggled("Ubuntu".to_string(), true));

    assert_eq!(event, None);
}

#[test]
fn a_failed_lookup_shows_why_and_clears_the_list() {
    let mut state = found();

    let _ = update(
        &mut state,
        Message::Found(Err("There are no running distributions.".to_string())),
    );

    assert_eq!(
        state.error.as_deref(),
        Some("There are no running distributions.")
    );
    assert!(state.plan.is_none());
    assert!(state.chosen.is_empty());
}

#[test]
fn importing_with_no_result_yet_does_nothing() {
    let mut state = State::default();

    let (_, event) = update(&mut state, Message::ImportPressed);

    assert_eq!(event, None);
}

#[test]
fn back_closes_the_screen() {
    let mut state = found();

    let (_, event) = update(&mut state, Message::BackPressed);

    assert_eq!(event, Some(Event::Close));
}

#[test]
fn another_lookup_is_not_started_while_one_runs() {
    let mut state = State {
        loading: true,
        ..State::default()
    };

    let _ = update(&mut state, Message::RefreshPressed);

    assert!(state.loading);
    assert!(state.plan.is_none());
}
