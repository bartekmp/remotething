//! What the user does in front of the remote desktop, as RDP input events.

use ironrdp_pdu::input::MousePdu;
use ironrdp_pdu::input::fast_path::{FastPathInputEvent, KeyboardFlags};
use ironrdp_pdu::input::mouse::PointerFlags;

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// The primary button.
    Left,
    /// The secondary button.
    Right,
    /// The wheel button.
    Middle,
}

/// One thing the user did. Positions are pixels of the remote desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// The wheel turned. Positive scrolls up, one step is 120 units like on Windows.
    Wheel {
        /// How far the wheel turned.
        delta: i16,
        /// Pixels from the left edge.
        x: u16,
        /// Pixels from the top edge.
        y: u16,
    },
    /// The window around the desktop changed size; the server is asked to match it.
    Resize {
        /// The wanted width in pixels.
        width: u16,
        /// The wanted height in pixels.
        height: u16,
    },
    /// A key went down or up, by its scancode (the position of the key, not its letter).
    Key {
        /// The set-1 scancode without the extended prefix.
        code: u8,
        /// Whether the key sends the extended prefix (right Ctrl, arrows and others).
        extended: bool,
        /// True for down, false for up.
        pressed: bool,
    },
}

/// The RDP events for one input.
pub(crate) fn to_events(input: Input) -> Vec<FastPathInputEvent> {
    match input {
        // Not an input event: the session sends it over its own channel.
        Input::Resize { .. } => Vec::new(),
        Input::MouseMoved { x, y } => vec![mouse(PointerFlags::MOVE, 0, x, y)],
        Input::Button {
            button,
            pressed,
            x,
            y,
        } => {
            let mut flags = match button {
                Button::Left => PointerFlags::LEFT_BUTTON,
                Button::Right => PointerFlags::RIGHT_BUTTON,
                Button::Middle => PointerFlags::MIDDLE_BUTTON_OR_WHEEL,
            };
            if pressed {
                flags |= PointerFlags::DOWN;
            }
            vec![mouse(flags, 0, x, y)]
        }
        Input::Wheel { delta, x, y } => {
            let mut flags = PointerFlags::VERTICAL_WHEEL;
            // The protocol sends the size as a magnitude and the direction as a flag.
            if delta < 0 {
                flags |= PointerFlags::WHEEL_NEGATIVE;
            }
            vec![mouse(flags, delta.saturating_abs(), x, y)]
        }
        Input::Key {
            code,
            extended,
            pressed,
        } => {
            let mut flags = KeyboardFlags::empty();
            if !pressed {
                flags |= KeyboardFlags::RELEASE;
            }
            if extended {
                flags |= KeyboardFlags::EXTENDED;
            }
            vec![FastPathInputEvent::KeyboardEvent(flags, code)]
        }
    }
}

fn mouse(flags: PointerFlags, wheel: i16, x: u16, y: u16) -> FastPathInputEvent {
    FastPathInputEvent::MouseEvent(MousePdu {
        flags,
        number_of_wheel_rotation_units: wheel,
        x_position: x,
        y_position: y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(input: Input) -> FastPathInputEvent {
        let mut events = to_events(input);
        assert_eq!(events.len(), 1);
        events.remove(0)
    }

    fn pointer(event: FastPathInputEvent) -> MousePdu {
        match event {
            FastPathInputEvent::MouseEvent(pdu) => pdu,
            other => panic!("expected a mouse event, got {other:?}"),
        }
    }

    #[test]
    fn a_move_carries_the_position() {
        let pdu = pointer(only(Input::MouseMoved { x: 10, y: 20 }));

        assert_eq!(pdu.flags, PointerFlags::MOVE);
        assert_eq!((pdu.x_position, pdu.y_position), (10, 20));
    }

    #[test]
    fn a_button_press_sets_down_and_a_release_does_not() {
        let button = |pressed| Input::Button {
            button: Button::Right,
            pressed,
            x: 1,
            y: 2,
        };
        let (press, release) = (button(true), button(false));

        let down = pointer(only(press)).flags;
        let up = pointer(only(release)).flags;

        assert_eq!(down, PointerFlags::RIGHT_BUTTON | PointerFlags::DOWN);
        assert_eq!(up, PointerFlags::RIGHT_BUTTON);
    }

    #[test]
    fn a_wheel_down_is_a_magnitude_with_the_negative_flag() {
        let pdu = pointer(only(Input::Wheel {
            delta: -120,
            x: 0,
            y: 0,
        }));

        assert_eq!(
            pdu.flags,
            PointerFlags::VERTICAL_WHEEL | PointerFlags::WHEEL_NEGATIVE
        );
        assert_eq!(pdu.number_of_wheel_rotation_units, 120);
    }

    #[test]
    fn keys_carry_scancode_release_and_extended_flags() {
        let down = only(Input::Key {
            code: 0x1D,
            extended: true,
            pressed: true,
        });
        let up = only(Input::Key {
            code: 0x1D,
            extended: false,
            pressed: false,
        });

        assert_eq!(
            down,
            FastPathInputEvent::KeyboardEvent(KeyboardFlags::EXTENDED, 0x1D)
        );
        assert_eq!(
            up,
            FastPathInputEvent::KeyboardEvent(KeyboardFlags::RELEASE, 0x1D)
        );
    }
}
