//! What the user does in front of the remote desktop.

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// The primary button.
    Left,
    /// The wheel button.
    Middle,
    /// The secondary button.
    Right,
}

/// One thing the user did. Positions are pixels of the remote desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// The pointer moved.
    MouseMoved {
        /// Pixels from the left edge.
        x: u16,
        /// Pixels from the top edge.
        y: u16,
    },
    /// A mouse button went down or up.
    Button {
        /// Which button.
        button: Button,
        /// True for down, false for up.
        pressed: bool,
        /// Pixels from the left edge.
        x: u16,
        /// Pixels from the top edge.
        y: u16,
    },
    /// The wheel turned: positive scrolls up, one step is 120 units like on Windows.
    Wheel {
        /// How far the wheel turned.
        delta: i16,
        /// Pixels from the left edge.
        x: u16,
        /// Pixels from the top edge.
        y: u16,
    },
    /// A key went down or up, by its X11 keysym.
    Key {
        /// The keysym, for example 0x61 for `a`.
        keysym: u32,
        /// True for down, false for up.
        pressed: bool,
    },
    /// The text on the local clipboard changed.
    Clipboard(String),
}

/// The bit of a button in the pointer event of the protocol.
pub(crate) fn mask(button: Button) -> u8 {
    match button {
        Button::Left => 1,
        Button::Middle => 2,
        Button::Right => 4,
    }
}

/// The bit that a wheel notch in this direction presses and releases.
pub(crate) const WHEEL_UP: u8 = 8;
/// The bit that a wheel notch down presses and releases.
pub(crate) const WHEEL_DOWN: u8 = 16;

/// Where the pointer is and which buttons are down: what a pointer event of
/// the protocol carries, which is the whole state every time.
#[derive(Debug, Default)]
pub(crate) struct Pointer {
    x: u16,
    y: u16,
    buttons: u8,
}

/// One pointer event of the protocol: position and the buttons that are down.
pub(crate) type PointerEvent = (u16, u16, u8);

impl Pointer {
    /// The pointer events to send for a mouse input; none for other inputs.
    pub(crate) fn events(&mut self, input: &Input) -> Vec<PointerEvent> {
        match input {
            Input::MouseMoved { x, y } => {
                self.place(*x, *y);
                vec![self.now()]
            }
            Input::Button {
                button,
                pressed,
                x,
                y,
            } => {
                self.place(*x, *y);
                if *pressed {
                    self.buttons |= mask(*button);
                } else {
                    self.buttons &= !mask(*button);
                }
                vec![self.now()]
            }
            Input::Wheel { delta, x, y } if *delta != 0 => {
                self.place(*x, *y);
                // A wheel notch is a button that is pressed and let go at once.
                let bit = if *delta > 0 { WHEEL_UP } else { WHEEL_DOWN };
                let pressed = (self.x, self.y, self.buttons | bit);
                vec![pressed, self.now()]
            }
            _ => Vec::new(),
        }
    }

    fn place(&mut self, x: u16, y: u16) {
        self.x = x;
        self.y = y;
    }

    fn now(&self) -> PointerEvent {
        (self.x, self.y, self.buttons)
    }
}

/// `text` as Latin-1, the only characters the protocol's clipboard carries;
/// others become `?`.
pub(crate) fn latin1(text: &str) -> String {
    text.chars()
        .map(|character| {
            if u32::from(character) < 256 {
                character
            } else {
                '?'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_have_the_bits_the_protocol_gives_them() {
        assert_eq!(mask(Button::Left), 1);
        assert_eq!(mask(Button::Middle), 2);
        assert_eq!(mask(Button::Right), 4);
    }

    #[test]
    fn the_wheel_bits_do_not_clash_with_buttons() {
        let buttons = mask(Button::Left) | mask(Button::Middle) | mask(Button::Right);

        assert_eq!(buttons & (WHEEL_UP | WHEEL_DOWN), 0);
    }

    #[test]
    fn a_press_keeps_the_button_down_until_the_release() {
        let mut pointer = Pointer::default();
        let press = Input::Button {
            button: Button::Left,
            pressed: true,
            x: 5,
            y: 6,
        };
        let release = Input::Button {
            button: Button::Left,
            pressed: false,
            x: 5,
            y: 6,
        };

        assert_eq!(pointer.events(&press), [(5, 6, 1)]);
        // Moving with the button down is a drag.
        assert_eq!(
            pointer.events(&Input::MouseMoved { x: 9, y: 9 }),
            [(9, 9, 1)]
        );
        assert_eq!(pointer.events(&release), [(5, 6, 0)]);
    }

    #[test]
    fn two_buttons_are_tracked_together() {
        let mut pointer = Pointer::default();
        let down = |button| Input::Button {
            button,
            pressed: true,
            x: 0,
            y: 0,
        };

        pointer.events(&down(Button::Left));
        let both = pointer.events(&down(Button::Right));

        assert_eq!(both, [(0, 0, 5)]);
    }

    #[test]
    fn a_wheel_notch_presses_and_releases_its_bit() {
        let mut pointer = Pointer::default();

        let up = pointer.events(&Input::Wheel {
            delta: 120,
            x: 1,
            y: 2,
        });
        let down = pointer.events(&Input::Wheel {
            delta: -120,
            x: 1,
            y: 2,
        });

        assert_eq!(up, [(1, 2, WHEEL_UP), (1, 2, 0)]);
        assert_eq!(down, [(1, 2, WHEEL_DOWN), (1, 2, 0)]);
    }

    #[test]
    fn keys_and_clipboard_are_not_pointer_events() {
        let mut pointer = Pointer::default();

        assert!(
            pointer
                .events(&Input::Key {
                    keysym: 0x61,
                    pressed: true
                })
                .is_empty()
        );
        assert!(
            pointer
                .events(&Input::Clipboard("x".to_string()))
                .is_empty()
        );
    }

    #[test]
    fn only_latin_1_survives_in_the_clipboard() {
        assert_eq!(latin1("café"), "café");
        assert_eq!(latin1("zażółć"), "za?ó??");
    }
}
