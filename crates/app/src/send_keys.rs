//! "Send keys" in the status bar of a remote desktop: key combinations the local system
//! would swallow (Alt+Tab, Win+L) or cannot type (Ctrl+Alt+Del), sent from a menu. The
//! creator builds others from modifier chips, a key list and a "press the keys" capture.

use std::fmt;

use iced::keyboard::Modifiers;
use iced::keyboard::key::{Code, Physical};

/// How long a combination is held; Windows ignores a shorter Ctrl+Alt+Del.
pub(crate) const HOLD: std::time::Duration = std::time::Duration::from_millis(300);

/// A key that can be sent: where it is (for RDP) and what it makes (for VNC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) code: Code,
    pub(crate) label: &'static str,
    /// The X11 keysym, which VNC sends.
    pub(crate) keysym: u32,
}

impl fmt::Display for Entry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label)
    }
}

const fn entry(code: Code, label: &'static str, keysym: u32) -> Entry {
    Entry {
        code,
        label,
        keysym,
    }
}

const CTRL: Entry = entry(Code::ControlLeft, "Ctrl", 0xffe3);
const ALT: Entry = entry(Code::AltLeft, "Alt", 0xffe9);
const SHIFT: Entry = entry(Code::ShiftLeft, "Shift", 0xffe1);
const WIN: Entry = entry(Code::SuperLeft, "Win", 0xffeb);

/// Keys of the key list, in the order shown: letters, digits, then the others.
const KEYS: &[Entry] = &[
    entry(Code::KeyA, "A", 0x61),
    entry(Code::KeyB, "B", 0x62),
    entry(Code::KeyC, "C", 0x63),
    entry(Code::KeyD, "D", 0x64),
    entry(Code::KeyE, "E", 0x65),
    entry(Code::KeyF, "F", 0x66),
    entry(Code::KeyG, "G", 0x67),
    entry(Code::KeyH, "H", 0x68),
    entry(Code::KeyI, "I", 0x69),
    entry(Code::KeyJ, "J", 0x6a),
    entry(Code::KeyK, "K", 0x6b),
    entry(Code::KeyL, "L", 0x6c),
    entry(Code::KeyM, "M", 0x6d),
    entry(Code::KeyN, "N", 0x6e),
    entry(Code::KeyO, "O", 0x6f),
    entry(Code::KeyP, "P", 0x70),
    entry(Code::KeyQ, "Q", 0x71),
    entry(Code::KeyR, "R", 0x72),
    entry(Code::KeyS, "S", 0x73),
    entry(Code::KeyT, "T", 0x74),
    entry(Code::KeyU, "U", 0x75),
    entry(Code::KeyV, "V", 0x76),
    entry(Code::KeyW, "W", 0x77),
    entry(Code::KeyX, "X", 0x78),
    entry(Code::KeyY, "Y", 0x79),
    entry(Code::KeyZ, "Z", 0x7a),
    entry(Code::Digit0, "0", 0x30),
    entry(Code::Digit1, "1", 0x31),
    entry(Code::Digit2, "2", 0x32),
    entry(Code::Digit3, "3", 0x33),
    entry(Code::Digit4, "4", 0x34),
    entry(Code::Digit5, "5", 0x35),
    entry(Code::Digit6, "6", 0x36),
    entry(Code::Digit7, "7", 0x37),
    entry(Code::Digit8, "8", 0x38),
    entry(Code::Digit9, "9", 0x39),
    entry(Code::F1, "F1", 0xffbe),
    entry(Code::F2, "F2", 0xffbf),
    entry(Code::F3, "F3", 0xffc0),
    entry(Code::F4, "F4", 0xffc1),
    entry(Code::F5, "F5", 0xffc2),
    entry(Code::F6, "F6", 0xffc3),
    entry(Code::F7, "F7", 0xffc4),
    entry(Code::F8, "F8", 0xffc5),
    entry(Code::F9, "F9", 0xffc6),
    entry(Code::F10, "F10", 0xffc7),
    entry(Code::F11, "F11", 0xffc8),
    entry(Code::F12, "F12", 0xffc9),
    entry(Code::Escape, "Esc", 0xff1b),
    entry(Code::Tab, "Tab", 0xff09),
    entry(Code::Enter, "Enter", 0xff0d),
    entry(Code::Space, "Space", 0x20),
    entry(Code::Backspace, "Backspace", 0xff08),
    entry(Code::Delete, "Delete", 0xffff),
    entry(Code::Insert, "Insert", 0xff63),
    entry(Code::Home, "Home", 0xff50),
    entry(Code::End, "End", 0xff57),
    entry(Code::PageUp, "Page Up", 0xff55),
    entry(Code::PageDown, "Page Down", 0xff56),
    entry(Code::ArrowLeft, "Left", 0xff51),
    entry(Code::ArrowUp, "Up", 0xff52),
    entry(Code::ArrowRight, "Right", 0xff53),
    entry(Code::ArrowDown, "Down", 0xff54),
    entry(Code::PrintScreen, "Print Screen", 0xff61),
];

/// The sendable key at this place, modifiers included.
fn find(code: Code) -> Option<Entry> {
    [CTRL, ALT, SHIFT, WIN]
        .into_iter()
        .chain(KEYS.iter().copied())
        .find(|known| known.code == code)
}

/// The X11 keysym VNC sends for a key of a combination.
pub(crate) fn keysym(code: Code) -> Option<u32> {
    find(code).map(|known| known.keysym)
}

/// A combination: its keys in the order they go down.
pub(crate) type Combo = Vec<Code>;

fn combo(keys: &[Entry]) -> Combo {
    keys.iter().map(|key| key.code).collect()
}

/// The combinations of the menu.
fn presets() -> Vec<Combo> {
    let key = |label| {
        *KEYS
            .iter()
            .find(|known| known.label == label)
            .expect("a key of the list")
    };
    vec![
        combo(&[CTRL, ALT, key("Delete")]),
        combo(&[CTRL, SHIFT, key("Esc")]),
        combo(&[ALT, key("Tab")]),
        combo(&[ALT, key("F4")]),
        combo(&[WIN]),
        combo(&[WIN, key("D")]),
        combo(&[WIN, key("E")]),
        combo(&[WIN, key("R")]),
        combo(&[WIN, key("L")]),
        combo(&[key("Print Screen")]),
    ]
}

/// "Ctrl+Alt+Delete": the name of a combination.
pub(crate) fn name(combo: &[Code]) -> String {
    let labels: Vec<&str> = combo
        .iter()
        .filter_map(|code| find(*code))
        .map(|known| known.label)
        .collect();
    labels.join("+")
}

/// The modifier chips of the creator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Win,
}

pub(crate) const MODIFIERS: [Modifier; 4] = [
    Modifier::Ctrl,
    Modifier::Alt,
    Modifier::Shift,
    Modifier::Win,
];

impl Modifier {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Ctrl => CTRL.label,
            Self::Alt => ALT.label,
            Self::Shift => SHIFT.label,
            Self::Win => WIN.label,
        }
    }

    fn entry(self) -> Entry {
        match self {
            Self::Ctrl => CTRL,
            Self::Alt => ALT,
            Self::Shift => SHIFT,
            Self::Win => WIN,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// The "Send keys" button.
    Toggled,
    /// A combination of the menu, by its place in it.
    PresetPressed(usize),
    /// "Custom…".
    CustomPressed,
    /// Back from the creator to the menu.
    BackPressed,
    ModifierToggled(Modifier),
    /// The text of the key field changed.
    KeyEdited(String),
    /// Reads the next keys pressed instead of sending them.
    CaptureToggled,
    SendPressed,
}

/// What the creator holds so far.
pub(crate) struct Draft {
    modifiers: [bool; 4],
    /// What the key field says; the key is what it names.
    pub(crate) text: String,
    pub(crate) capturing: bool,
}

impl Draft {
    fn new() -> Self {
        Self {
            modifiers: [false; 4],
            text: String::new(),
            capturing: false,
        }
    }

    /// The key the field names, ignoring case and spaces: `f4`, `page up`.
    pub(crate) fn key(&self) -> Option<Entry> {
        let wanted = self.text.trim();
        KEYS.iter()
            .copied()
            .find(|known| known.label.eq_ignore_ascii_case(wanted))
    }

    pub(crate) fn has(&self, modifier: Modifier) -> bool {
        self.modifiers[modifier as usize]
    }

    fn set(&mut self, modifier: Modifier, on: bool) {
        self.modifiers[modifier as usize] = on;
    }

    /// The keys chosen, modifiers first; `None` while nothing is chosen.
    pub(crate) fn combo(&self) -> Option<Combo> {
        let mut keys: Vec<Entry> = MODIFIERS
            .into_iter()
            .filter(|modifier| self.has(*modifier))
            .map(Modifier::entry)
            .collect();
        keys.extend(self.key());
        (!keys.is_empty()).then(|| combo(&keys))
    }
}

/// Whether the menu is open, and the creator if it is showing.
#[derive(Default)]
pub(crate) struct State {
    pub(crate) open: bool,
    pub(crate) draft: Option<Draft>,
}

impl State {
    /// Whether keys go to the menu instead of the remote desktop.
    pub(crate) fn wants_keys(&self) -> bool {
        self.draft.is_some()
    }

    pub(crate) fn presets(&self) -> Vec<Combo> {
        presets()
    }

    /// A key was pressed while the creator reads keys. Modifiers wait for the key they go with.
    /// The answer says whether the key was read, and so belongs to no one else.
    pub(crate) fn capture(&mut self, physical: Physical, modifiers: Modifiers) -> bool {
        let Some(draft) = self.draft.as_mut().filter(|draft| draft.capturing) else {
            return false;
        };
        let Physical::Code(code) = physical else {
            return true;
        };
        let Some(pressed) = find(code) else {
            return true;
        };
        if let Some(modifier) = MODIFIERS.into_iter().find(|m| m.entry().code == code) {
            draft.set(modifier, true);
            return true;
        }
        draft.set(
            Modifier::Ctrl,
            modifiers.control() || draft.has(Modifier::Ctrl),
        );
        draft.set(Modifier::Alt, modifiers.alt() || draft.has(Modifier::Alt));
        draft.set(
            Modifier::Shift,
            modifiers.shift() || draft.has(Modifier::Shift),
        );
        draft.set(Modifier::Win, modifiers.logo() || draft.has(Modifier::Win));
        draft.text = pressed.label.to_string();
        draft.capturing = false;
        true
    }
}

/// Applies a message; the answer is a combination to send now.
pub(crate) fn update(state: &mut State, message: Message) -> Option<Combo> {
    match message {
        Message::Toggled => {
            state.open = !state.open;
            state.draft = None;
        }
        Message::PresetPressed(index) => {
            state.open = false;
            return state.presets().into_iter().nth(index);
        }
        Message::CustomPressed => state.draft = Some(Draft::new()),
        Message::BackPressed => state.draft = None,
        Message::ModifierToggled(modifier) => {
            if let Some(draft) = &mut state.draft {
                draft.set(modifier, !draft.has(modifier));
            }
        }
        Message::KeyEdited(text) => {
            if let Some(draft) = &mut state.draft {
                draft.text = text;
            }
        }
        Message::CaptureToggled => {
            if let Some(draft) = &mut state.draft {
                draft.capturing = !draft.capturing;
            }
        }
        Message::SendPressed => {
            let combo = state.draft.as_ref().and_then(Draft::combo)?;
            state.open = false;
            state.draft = None;
            return Some(combo);
        }
    }
    None
}

#[cfg(test)]
#[path = "send_keys_tests.rs"]
mod tests;
