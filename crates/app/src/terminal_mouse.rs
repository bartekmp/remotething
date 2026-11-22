//! Mouse reports: what a terminal sends when a program such as tmux or htop asks to
//! be told about clicks, drags and the wheel.

use alacritty_terminal::term::TermMode;

/// The mouse buttons programs can be told about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

impl Button {
    fn code(self) -> u8 {
        match self {
            Button::Left => 0,
            Button::Middle => 1,
            Button::Right => 2,
        }
    }
}

/// What the mouse did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Press(Button),
    Release(Button),
    /// The pointer moved to another character with the button held.
    Drag(Button),
    /// The pointer moved to another character with no button held.
    Move,
    /// The wheel turned by this many notches, away from the user when `up`.
    Wheel {
        up: bool,
        notches: u8,
    },
}

/// A mouse event on one character of the screen, counted from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub action: Action,
    pub column: usize,
    pub line: usize,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// What the remote program asked to be told, from the terminal modes it switched on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mode {
    /// Presses, releases and the wheel.
    pub clicks: bool,
    /// Moves with a button held.
    pub drags: bool,
    /// Every move.
    pub motion: bool,
    /// The `CSI <` form, which has no limit on the position.
    pub sgr: bool,
}

impl Mode {
    pub fn of(modes: TermMode) -> Self {
        Self {
            clicks: modes.intersects(TermMode::MOUSE_MODE),
            drags: modes.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION),
            motion: modes.contains(TermMode::MOUSE_MOTION),
            sgr: modes.contains(TermMode::SGR_MOUSE),
        }
    }

    /// Whether the program wants mouse events at all.
    pub fn reporting(self) -> bool {
        self.clicks
    }
}

/// The largest position the old form can say: one byte, starting at 32.
const LEGACY_LIMIT: usize = 222;

/// The bytes for the program, or `None` when it did not ask for this kind of event.
pub fn encode(report: &Report, mode: Mode) -> Option<Vec<u8>> {
    let wanted = match report.action {
        Action::Move => mode.motion,
        Action::Drag(_) => mode.drags,
        _ => mode.clicks,
    };
    if !wanted {
        return None;
    }
    let modifiers =
        u8::from(report.shift) * 4 + u8::from(report.alt) * 8 + u8::from(report.ctrl) * 16;
    let (code, released, times) = match report.action {
        Action::Press(button) => (button.code(), false, 1),
        // The old form cannot say which button was released.
        Action::Release(button) => (if mode.sgr { button.code() } else { 3 }, true, 1),
        Action::Drag(button) => (button.code() + 32, false, 1),
        Action::Move => (3 + 32, false, 1),
        Action::Wheel { up, notches } => (if up { 64 } else { 65 }, false, notches),
    };
    let code = code + modifiers;
    let (column, line) = (report.column + 1, report.line + 1);

    let one = if mode.sgr {
        let end = if released { 'm' } else { 'M' };
        format!("\x1b[<{code};{column};{line}{end}").into_bytes()
    } else {
        if column > LEGACY_LIMIT || line > LEGACY_LIMIT {
            return None;
        }
        // Each value is sent as a byte, offset by 32 so it never is a control character.
        vec![
            0x1b,
            b'[',
            b'M',
            code + 32,
            column as u8 + 32,
            line as u8 + 32,
        ]
    };
    Some(one.repeat(usize::from(times)))
}

#[cfg(test)]
#[path = "terminal_mouse_tests.rs"]
mod tests;
