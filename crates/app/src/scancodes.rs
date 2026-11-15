//! Keyboard keys as the scancodes a remote Windows desktop expects.
//!
//! RDP sends the position of the key, not the letter, so the remote keyboard
//! layout decides what is typed. These are PC set-1 scancodes.

use iced::keyboard::key::{Code, Physical};

/// A scancode and whether it carries the extended prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scancode {
    pub code: u8,
    pub extended: bool,
}

const fn plain(code: u8) -> Option<Scancode> {
    Some(Scancode {
        code,
        extended: false,
    })
}

const fn extended(code: u8) -> Option<Scancode> {
    Some(Scancode {
        code,
        extended: true,
    })
}

/// The scancode of a key, or `None` for keys the remote side has no use for.
pub fn of(key: Physical) -> Option<Scancode> {
    let Physical::Code(code) = key else {
        return None;
    };
    match code {
        Code::Escape => plain(0x01),
        Code::Digit1 => plain(0x02),
        Code::Digit2 => plain(0x03),
        Code::Digit3 => plain(0x04),
        Code::Digit4 => plain(0x05),
        Code::Digit5 => plain(0x06),
        Code::Digit6 => plain(0x07),
        Code::Digit7 => plain(0x08),
        Code::Digit8 => plain(0x09),
        Code::Digit9 => plain(0x0A),
        Code::Digit0 => plain(0x0B),
        Code::Minus => plain(0x0C),
        Code::Equal => plain(0x0D),
        Code::Backspace => plain(0x0E),
        Code::Tab => plain(0x0F),
        Code::KeyQ => plain(0x10),
        Code::KeyW => plain(0x11),
        Code::KeyE => plain(0x12),
        Code::KeyR => plain(0x13),
        Code::KeyT => plain(0x14),
        Code::KeyY => plain(0x15),
        Code::KeyU => plain(0x16),
        Code::KeyI => plain(0x17),
        Code::KeyO => plain(0x18),
        Code::KeyP => plain(0x19),
        Code::BracketLeft => plain(0x1A),
        Code::BracketRight => plain(0x1B),
        Code::Enter => plain(0x1C),
        Code::ControlLeft => plain(0x1D),
        Code::KeyA => plain(0x1E),
        Code::KeyS => plain(0x1F),
        Code::KeyD => plain(0x20),
        Code::KeyF => plain(0x21),
        Code::KeyG => plain(0x22),
        Code::KeyH => plain(0x23),
        Code::KeyJ => plain(0x24),
        Code::KeyK => plain(0x25),
        Code::KeyL => plain(0x26),
        Code::Semicolon => plain(0x27),
        Code::Quote => plain(0x28),
        Code::Backquote => plain(0x29),
        Code::ShiftLeft => plain(0x2A),
        Code::Backslash => plain(0x2B),
        Code::KeyZ => plain(0x2C),
        Code::KeyX => plain(0x2D),
        Code::KeyC => plain(0x2E),
        Code::KeyV => plain(0x2F),
        Code::KeyB => plain(0x30),
        Code::KeyN => plain(0x31),
        Code::KeyM => plain(0x32),
        Code::Comma => plain(0x33),
        Code::Period => plain(0x34),
        Code::Slash => plain(0x35),
        Code::ShiftRight => plain(0x36),
        Code::NumpadMultiply | Code::NumpadStar => plain(0x37),
        Code::AltLeft => plain(0x38),
        Code::Space => plain(0x39),
        Code::CapsLock => plain(0x3A),
        Code::F1 => plain(0x3B),
        Code::F2 => plain(0x3C),
        Code::F3 => plain(0x3D),
        Code::F4 => plain(0x3E),
        Code::F5 => plain(0x3F),
        Code::F6 => plain(0x40),
        Code::F7 => plain(0x41),
        Code::F8 => plain(0x42),
        Code::F9 => plain(0x43),
        Code::F10 => plain(0x44),
        Code::NumLock => plain(0x45),
        Code::ScrollLock => plain(0x46),
        Code::Numpad7 => plain(0x47),
        Code::Numpad8 => plain(0x48),
        Code::Numpad9 => plain(0x49),
        Code::NumpadSubtract => plain(0x4A),
        Code::Numpad4 => plain(0x4B),
        Code::Numpad5 => plain(0x4C),
        Code::Numpad6 => plain(0x4D),
        Code::NumpadAdd => plain(0x4E),
        Code::Numpad1 => plain(0x4F),
        Code::Numpad2 => plain(0x50),
        Code::Numpad3 => plain(0x51),
        Code::Numpad0 => plain(0x52),
        Code::NumpadDecimal => plain(0x53),
        Code::IntlBackslash => plain(0x56),
        Code::F11 => plain(0x57),
        Code::F12 => plain(0x58),

        Code::NumpadEnter => extended(0x1C),
        Code::ControlRight => extended(0x1D),
        Code::NumpadDivide => extended(0x35),
        Code::PrintScreen => extended(0x37),
        Code::AltRight => extended(0x38),
        Code::Home => extended(0x47),
        Code::ArrowUp => extended(0x48),
        Code::PageUp => extended(0x49),
        Code::ArrowLeft => extended(0x4B),
        Code::ArrowRight => extended(0x4D),
        Code::End => extended(0x4F),
        Code::ArrowDown => extended(0x50),
        Code::PageDown => extended(0x51),
        Code::Insert => extended(0x52),
        Code::Delete => extended(0x53),
        Code::SuperLeft => extended(0x5B),
        Code::SuperRight => extended(0x5C),
        Code::ContextMenu => extended(0x5D),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use iced::keyboard::key::NativeCode;

    use super::*;

    fn code(code: Code) -> Option<Scancode> {
        of(Physical::Code(code))
    }

    #[test]
    fn letters_digits_and_modifiers_have_their_pc_scancodes() {
        assert_eq!(code(Code::KeyA), plain(0x1E));
        assert_eq!(code(Code::Digit1), plain(0x02));
        assert_eq!(code(Code::ShiftLeft), plain(0x2A));
        assert_eq!(code(Code::F12), plain(0x58));
    }

    #[test]
    fn arrows_and_the_right_hand_modifiers_are_extended() {
        assert_eq!(code(Code::ArrowUp), extended(0x48));
        assert_eq!(code(Code::ControlRight), extended(0x1D));
        // The same scancode without the prefix is a different key.
        assert_eq!(code(Code::ControlLeft), plain(0x1D));
        assert_eq!(code(Code::NumpadEnter), extended(0x1C));
    }

    #[test]
    fn keys_without_a_place_on_a_pc_keyboard_are_ignored() {
        assert_eq!(code(Code::AudioVolumeUp), None);
        assert_eq!(of(Physical::Unidentified(NativeCode::Unidentified)), None);
    }
}
