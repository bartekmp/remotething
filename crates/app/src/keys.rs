//! Turns key presses into the bytes a remote shell expects.

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};

/// The bytes for a key press, or `None` for keys a terminal does not send.
///
/// `application_cursor` is the mode full-screen programs switch on to get
/// arrow keys in a different form.
pub fn to_bytes(
    key: &Key,
    modifiers: Modifiers,
    text: Option<&str>,
    application_cursor: bool,
) -> Option<Vec<u8>> {
    if let Key::Named(named) = key
        && let Some(bytes) = named_key(*named, application_cursor)
    {
        return Some(bytes.to_vec());
    }

    if modifiers.control()
        && let Key::Character(character) = key
    {
        return control_code(character.chars().next()?).map(|code| vec![code]);
    }

    let typed = text.filter(|typed| !typed.is_empty())?;
    let mut bytes = Vec::new();
    if modifiers.alt() {
        // Alt is sent as an Escape in front of the key.
        bytes.push(0x1b);
    }
    bytes.extend_from_slice(typed.as_bytes());
    Some(bytes)
}

fn named_key(named: Named, application_cursor: bool) -> Option<&'static [u8]> {
    let bytes: &[u8] = match (named, application_cursor) {
        (Named::Enter, _) => b"\r",
        (Named::Backspace, _) => b"\x7f",
        (Named::Tab, _) => b"\t",
        (Named::Escape, _) => b"\x1b",
        (Named::ArrowUp, false) => b"\x1b[A",
        (Named::ArrowDown, false) => b"\x1b[B",
        (Named::ArrowRight, false) => b"\x1b[C",
        (Named::ArrowLeft, false) => b"\x1b[D",
        (Named::ArrowUp, true) => b"\x1bOA",
        (Named::ArrowDown, true) => b"\x1bOB",
        (Named::ArrowRight, true) => b"\x1bOC",
        (Named::ArrowLeft, true) => b"\x1bOD",
        (Named::Home, _) => b"\x1b[H",
        (Named::End, _) => b"\x1b[F",
        (Named::Insert, _) => b"\x1b[2~",
        (Named::Delete, _) => b"\x1b[3~",
        (Named::PageUp, _) => b"\x1b[5~",
        (Named::PageDown, _) => b"\x1b[6~",
        (Named::F1, _) => b"\x1bOP",
        (Named::F2, _) => b"\x1bOQ",
        (Named::F3, _) => b"\x1bOR",
        (Named::F4, _) => b"\x1bOS",
        (Named::F5, _) => b"\x1b[15~",
        (Named::F6, _) => b"\x1b[17~",
        (Named::F7, _) => b"\x1b[18~",
        (Named::F8, _) => b"\x1b[19~",
        (Named::F9, _) => b"\x1b[20~",
        (Named::F10, _) => b"\x1b[21~",
        (Named::F11, _) => b"\x1b[23~",
        (Named::F12, _) => b"\x1b[24~",
        _ => return None,
    };
    Some(bytes)
}

/// Ctrl with a letter is that letter's position in the alphabet; a few symbols have codes too.
fn control_code(character: char) -> Option<u8> {
    match character.to_ascii_lowercase() {
        letter @ 'a'..='z' => Some(letter as u8 - b'a' + 1),
        ' ' | '@' => Some(0),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(text: &str) -> Key {
        Key::Character(text.into())
    }

    #[test]
    fn printable_text_passes_through() {
        let bytes = to_bytes(&character("ą"), Modifiers::empty(), Some("ą"), false);

        assert_eq!(bytes, Some("ą".as_bytes().to_vec()));
    }

    #[test]
    fn enter_becomes_carriage_return() {
        let bytes = to_bytes(
            &Key::Named(Named::Enter),
            Modifiers::empty(),
            Some("\r"),
            false,
        );

        assert_eq!(bytes, Some(b"\r".to_vec()));
    }

    #[test]
    fn ctrl_c_becomes_the_interrupt_byte() {
        let bytes = to_bytes(&character("c"), Modifiers::CTRL, Some("\u{3}"), false);

        assert_eq!(bytes, Some(vec![3]));
    }

    #[test]
    fn ctrl_with_a_digit_sends_nothing() {
        assert_eq!(
            to_bytes(&character("1"), Modifiers::CTRL, None, false),
            None
        );
    }

    #[test]
    fn alt_puts_an_escape_in_front() {
        let bytes = to_bytes(&character("b"), Modifiers::ALT, Some("b"), false);

        assert_eq!(bytes, Some(b"\x1bb".to_vec()));
    }

    #[test]
    fn arrows_follow_the_cursor_key_mode() {
        let up = Key::Named(Named::ArrowUp);

        assert_eq!(
            to_bytes(&up, Modifiers::empty(), None, false),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            to_bytes(&up, Modifiers::empty(), None, true),
            Some(b"\x1bOA".to_vec())
        );
    }

    #[test]
    fn a_modifier_on_its_own_sends_nothing() {
        let shift = Key::Named(Named::Shift);

        assert_eq!(to_bytes(&shift, Modifiers::SHIFT, None, false), None);
    }

    #[test]
    fn the_space_bar_sends_a_space() {
        let space = Key::Named(Named::Space);

        assert_eq!(
            to_bytes(&space, Modifiers::empty(), Some(" "), false),
            Some(b" ".to_vec())
        );
    }
}
