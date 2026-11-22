use super::*;

const SGR: Mode = Mode {
    clicks: true,
    drags: false,
    motion: false,
    sgr: true,
};

fn at(action: Action, column: usize, line: usize) -> Report {
    Report {
        action,
        column,
        line,
        shift: false,
        alt: false,
        ctrl: false,
    }
}

#[test]
fn a_click_in_the_sgr_form_gives_the_position_from_one() {
    let press = at(Action::Press(Button::Left), 4, 9);
    let release = at(Action::Release(Button::Left), 4, 9);

    assert_eq!(encode(&press, SGR).unwrap(), b"\x1b[<0;5;10M");
    assert_eq!(encode(&release, SGR).unwrap(), b"\x1b[<0;5;10m");
}

#[test]
fn the_old_form_sends_bytes_and_cannot_name_the_released_button() {
    let mode = Mode { sgr: false, ..SGR };
    let press = at(Action::Press(Button::Right), 0, 0);
    let release = at(Action::Release(Button::Right), 0, 0);

    assert_eq!(encode(&press, mode).unwrap(), b"\x1b[M\"!!");
    assert_eq!(encode(&release, mode).unwrap(), b"\x1b[M#!!");
}

#[test]
fn the_old_form_drops_positions_it_cannot_say() {
    let mode = Mode { sgr: false, ..SGR };

    assert!(encode(&at(Action::Press(Button::Left), 222, 0), mode).is_none());
    assert!(encode(&at(Action::Press(Button::Left), 221, 0), mode).is_some());
    assert!(encode(&at(Action::Press(Button::Left), 300, 0), SGR).is_some());
}

#[test]
fn modifiers_add_to_the_button_code() {
    let report = Report {
        shift: true,
        alt: true,
        ctrl: true,
        ..at(Action::Press(Button::Middle), 0, 0)
    };

    assert_eq!(encode(&report, SGR).unwrap(), b"\x1b[<29;1;1M");
}

#[test]
fn the_wheel_sends_one_report_per_notch() {
    let up = at(
        Action::Wheel {
            up: true,
            notches: 2,
        },
        1,
        1,
    );
    let down = at(
        Action::Wheel {
            up: false,
            notches: 1,
        },
        1,
        1,
    );

    assert_eq!(encode(&up, SGR).unwrap(), b"\x1b[<64;2;2M\x1b[<64;2;2M");
    assert_eq!(encode(&down, SGR).unwrap(), b"\x1b[<65;2;2M");
}

#[test]
fn drags_and_moves_are_sent_only_when_asked_for() {
    let drag = at(Action::Drag(Button::Left), 0, 0);
    let motion = at(Action::Move, 0, 0);
    let dragging = Mode { drags: true, ..SGR };
    let everything = Mode {
        motion: true,
        ..dragging
    };

    assert!(encode(&drag, SGR).is_none());
    assert_eq!(encode(&drag, dragging).unwrap(), b"\x1b[<32;1;1M");
    assert!(encode(&motion, dragging).is_none());
    assert_eq!(encode(&motion, everything).unwrap(), b"\x1b[<35;1;1M");
}

#[test]
fn nothing_is_sent_when_the_program_did_not_ask() {
    let off = Mode::default();

    assert!(encode(&at(Action::Press(Button::Left), 0, 0), off).is_none());
    assert!(!off.reporting());
}

#[test]
fn the_terminal_modes_become_what_the_program_asked() {
    let click = Mode::of(TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE);
    let motion = Mode::of(TermMode::MOUSE_MOTION);

    assert!(click.reporting() && click.sgr && !click.drags);
    assert!(motion.reporting() && motion.drags && motion.motion && !motion.sgr);
    assert!(!Mode::of(TermMode::empty()).reporting());
}
