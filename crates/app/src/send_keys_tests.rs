//! Tests of `send_keys.rs`.

use super::*;

fn open_creator() -> State {
    let mut state = State::default();
    update(&mut state, Message::Toggled);
    update(&mut state, Message::CustomPressed);
    state
}

#[test]
fn a_preset_is_sent_and_closes_the_menu() {
    let mut state = State::default();
    update(&mut state, Message::Toggled);

    let sent = update(&mut state, Message::PresetPressed(0));

    assert_eq!(
        sent,
        Some(vec![Code::ControlLeft, Code::AltLeft, Code::Delete])
    );
    assert!(!state.open);
}

#[test]
fn every_preset_is_made_of_known_keys() {
    for combo in presets() {
        assert!(combo.iter().all(|code| keysym(*code).is_some()));
        assert!(!name(&combo).is_empty());
    }
}

#[test]
fn chips_and_a_key_make_a_combination_with_the_modifiers_first() {
    let mut state = open_creator();
    update(&mut state, Message::ModifierToggled(Modifier::Alt));
    update(&mut state, Message::ModifierToggled(Modifier::Ctrl));
    update(&mut state, Message::KeyEdited(" f4 ".into()));

    let sent = update(&mut state, Message::SendPressed).unwrap();

    assert_eq!(name(&sent), "Ctrl+Alt+F4");
    assert!(state.draft.is_none() && !state.open);
}

#[test]
fn nothing_is_sent_until_something_is_chosen() {
    let mut state = open_creator();

    assert_eq!(update(&mut state, Message::SendPressed), None);
    assert!(state.draft.is_some());
}

#[test]
fn a_chip_toggles_off_again() {
    let mut state = open_creator();
    update(&mut state, Message::ModifierToggled(Modifier::Win));
    update(&mut state, Message::ModifierToggled(Modifier::Win));

    assert_eq!(state.draft.unwrap().combo(), None);
}

#[test]
fn capturing_reads_the_modifiers_and_the_key_pressed() {
    let mut state = open_creator();
    update(&mut state, Message::CaptureToggled);

    assert!(state.capture(Physical::Code(Code::ControlLeft), Modifiers::CTRL));
    assert!(state.capture(
        Physical::Code(Code::KeyL),
        Modifiers::CTRL | Modifiers::LOGO,
    ));

    let draft = state.draft.unwrap();
    assert!(!draft.capturing);
    assert_eq!(name(&draft.combo().unwrap()), "Ctrl+Win+L");
}

#[test]
fn keys_are_ignored_unless_capturing() {
    let mut state = open_creator();

    assert!(!state.capture(Physical::Code(Code::KeyL), Modifiers::empty()));

    assert_eq!(state.draft.unwrap().combo(), None);
}

#[test]
fn the_creator_takes_the_keys_from_the_desktop() {
    let mut state = State::default();
    assert!(!state.wants_keys());
    update(&mut state, Message::CustomPressed);
    assert!(state.wants_keys());
    update(&mut state, Message::BackPressed);
    assert!(!state.wants_keys());
}

#[test]
fn the_key_field_names_a_key_in_any_case_and_a_wrong_name_names_none() {
    let mut state = open_creator();

    update(&mut state, Message::KeyEdited("page up".into()));
    assert_eq!(
        state.draft.as_ref().unwrap().key().unwrap().code,
        Code::PageUp
    );

    update(&mut state, Message::KeyEdited("pageup!".into()));
    assert_eq!(state.draft.as_ref().unwrap().key(), None);
}
