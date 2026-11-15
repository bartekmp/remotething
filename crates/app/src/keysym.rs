//! X11 keysyms for the keys of a VNC session, which wants the character a key
//! makes rather than the place of the key.

use iced::keyboard::Key;
use iced::keyboard::key::{Code, Named, Physical};

/// The keysym of a key press, or `None` for keys that have none.
///
/// `text` is what the key typed with the modifiers applied, so Shift+a gives `A`.
pub(crate) fn of(key: &Key, physical: &Physical, text: Option<&str>) -> Option<u32> {
    match key {
        Key::Named(named) => named_keysym(*named, physical),
        Key::Character(character) => {
            let typed = text
                .filter(|text| text.chars().next().is_some_and(|c| !c.is_control()))
                .unwrap_or(character.as_str());
            typed.chars().next().map(character_keysym)
        }
        Key::Unidentified => None,
    }
}

/// The keysym of a character: Latin-1 keeps its number, everything else is Unicode plus an offset.
fn character_keysym(character: char) -> u32 {
    let code = u32::from(character);
    if code < 0x100 {
        code
    } else {
        0x0100_0000 + code
    }
}

fn named_keysym(named: Named, physical: &Physical) -> Option<u32> {
    let right = |code: Code| matches!(physical, Physical::Code(pressed) if *pressed == code);
    let keysym = match named {
        Named::Space => 0x20,
        Named::Enter => 0xff0d,
        Named::Tab => 0xff09,
        Named::Backspace => 0xff08,
        Named::Escape => 0xff1b,
        Named::Delete => 0xffff,
        Named::Insert => 0xff63,
        Named::Home => 0xff50,
        Named::End => 0xff57,
        Named::PageUp => 0xff55,
        Named::PageDown => 0xff56,
        Named::ArrowLeft => 0xff51,
        Named::ArrowUp => 0xff52,
        Named::ArrowRight => 0xff53,
        Named::ArrowDown => 0xff54,
        Named::F1 => 0xffbe,
        Named::F2 => 0xffbf,
        Named::F3 => 0xffc0,
        Named::F4 => 0xffc1,
        Named::F5 => 0xffc2,
        Named::F6 => 0xffc3,
        Named::F7 => 0xffc4,
        Named::F8 => 0xffc5,
        Named::F9 => 0xffc6,
        Named::F10 => 0xffc7,
        Named::F11 => 0xffc8,
        Named::F12 => 0xffc9,
        Named::Shift if right(Code::ShiftRight) => 0xffe2,
        Named::Shift => 0xffe1,
        Named::Control if right(Code::ControlRight) => 0xffe4,
        Named::Control => 0xffe3,
        Named::Alt if right(Code::AltRight) => 0xffea,
        Named::Alt => 0xffe9,
        Named::AltGraph => 0xfe03,
        Named::Super | Named::Meta if right(Code::SuperRight) => 0xffec,
        Named::Super | Named::Meta => 0xffeb,
        Named::CapsLock => 0xffe5,
        Named::NumLock => 0xff7f,
        Named::ScrollLock => 0xff14,
        Named::ContextMenu => 0xff67,
        Named::PrintScreen => 0xff61,
        Named::Pause => 0xff13,
        _ => return None,
    };
    Some(keysym)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(text: &str) -> Key {
        Key::Character(text.into())
    }

    fn any() -> Physical {
        Physical::Unidentified(iced::keyboard::key::NativeCode::Unidentified)
    }

    #[test]
    fn a_letter_gives_its_ascii_code() {
        assert_eq!(of(&character("a"), &any(), Some("a")), Some(0x61));
    }

    #[test]
    fn the_typed_text_decides_the_case() {
        assert_eq!(of(&character("a"), &any(), Some("A")), Some(0x41));
    }

    #[test]
    fn a_control_character_in_the_text_is_not_used() {
        // Ctrl+C reports the control character in the text; the key itself is `c`.
        assert_eq!(of(&character("c"), &any(), Some("\u{3}")), Some(0x63));
    }

    #[test]
    fn latin_1_letters_keep_their_number() {
        assert_eq!(of(&character("é"), &any(), Some("é")), Some(0xe9));
    }

    #[test]
    fn other_characters_are_unicode_with_an_offset() {
        assert_eq!(of(&character("€"), &any(), Some("€")), Some(0x0100_20ac));
        assert_eq!(of(&character("ż"), &any(), Some("ż")), Some(0x0100_017c));
    }

    #[test]
    fn named_keys_have_their_own_keysyms() {
        assert_eq!(of(&Key::Named(Named::Enter), &any(), None), Some(0xff0d));
        assert_eq!(of(&Key::Named(Named::F5), &any(), None), Some(0xffc2));
        assert_eq!(
            of(&Key::Named(Named::ArrowLeft), &any(), None),
            Some(0xff51)
        );
        assert_eq!(of(&Key::Named(Named::Delete), &any(), None), Some(0xffff));
    }

    #[test]
    fn the_right_hand_modifiers_are_told_apart_by_the_place_of_the_key() {
        let left = of(
            &Key::Named(Named::Control),
            &Physical::Code(Code::ControlLeft),
            None,
        );
        let right = of(
            &Key::Named(Named::Control),
            &Physical::Code(Code::ControlRight),
            None,
        );

        assert_eq!((left, right), (Some(0xffe3), Some(0xffe4)));
    }

    #[test]
    fn a_key_without_a_meaning_gives_nothing() {
        assert_eq!(of(&Key::Unidentified, &any(), None), None);
        assert_eq!(of(&Key::Named(Named::BrowserBack), &any(), None), None);
    }
}
