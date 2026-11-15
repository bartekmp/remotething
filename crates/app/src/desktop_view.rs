//! Draws a Remote Desktop tab and turns the pointer over it into input.

use iced::widget::image::Handle;
use iced::widget::{button, column, container, image, mouse_area, responsive, row, text};
use iced::{ContentFit, Element, Font, Length, Point, Size, Theme, mouse};
use remotething_rdp::Button;

use crate::credentials::Hidden;
use crate::desktop::{Form, Live, Message, Stage, State};
use crate::i18n::fl;
use crate::theme;

const FORM_WIDTH: f32 = 320.0;

pub fn view(state: &State) -> Element<'_, Message> {
    match &state.stage {
        Stage::LoadingSecret | Stage::Connecting => centred(
            text(fl!(
                "session-connecting",
                host = state.connection.host.as_str()
            ))
            .style(theme::secondary_text),
        ),
        Stage::Asking(form) => centred(sign_in_form(&state.connection.name, form)),
        Stage::Certificate {
            certificate,
            changed,
        } => centred(certificate_question(
            &certificate.host,
            &certificate.fingerprint,
            *changed,
        )),
        Stage::Connected(live) => desktop(live),
        Stage::Ended(reason) => {
            let reconnect = button(text(fl!("session-reconnect")))
                .style(theme::primary)
                .on_press(Message::ReconnectPressed);
            centred(
                column![text(reason.as_str()), reconnect]
                    .spacing(theme::GAP)
                    .max_width(FORM_WIDTH),
            )
        }
    }
}

fn centred<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center(Length::Fill).into()
}

fn sign_in_form<'a>(name: &str, form: &'a Form) -> Element<'a, Message> {
    let mut fields = column![text(name.to_string()).size(theme::TEXT_TITLE)]
        .spacing(10)
        .max_width(FORM_WIDTH);
    if let Some(note) = &form.note {
        fields = fields.push(text(note.as_str()).style(text::danger));
    }
    if form.ask_username {
        fields = fields.push(
            theme::input(&fl!("session-username"), &form.username)
                .on_input(Message::UsernameChanged)
                .on_submit(Message::Submitted),
        );
    }
    fields = fields.push(
        theme::input(&fl!("session-password"), &form.password)
            .secure(true)
            .on_input(|typed| Message::PasswordChanged(Hidden(typed)))
            .on_submit(Message::Submitted),
    );
    fields
        .push(
            button(text(fl!("session-connect")))
                .style(theme::primary)
                .on_press(Message::Submitted),
        )
        .into()
}

fn certificate_question<'a>(
    host: &'a str,
    fingerprint: &'a str,
    changed: bool,
) -> Element<'a, Message> {
    let (explanation, trust_label) = if changed {
        (
            text(fl!("desktop-certificate-changed", host = host)).style(text::danger),
            fl!("session-trust-changed"),
        )
    } else {
        (
            text(fl!("desktop-certificate-new", host = host)),
            fl!("session-trust"),
        )
    };
    let shown = column![
        text(fl!("session-fingerprint"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        // A fingerprint has no spaces to break at.
        text(fingerprint)
            .font(Font::MONOSPACE)
            .wrapping(text::Wrapping::Glyph),
    ]
    .spacing(2);
    // A changed certificate is the dangerous case, so trusting it is not the highlighted action.
    let style: fn(&Theme, button::Status) -> button::Style = if changed {
        theme::danger
    } else {
        theme::primary
    };
    let trust = button(text(trust_label))
        .style(style)
        .on_press(Message::TrustPressed);
    column![explanation, shown, row![trust]]
        .spacing(theme::GAP)
        .max_width(FORM_WIDTH + 120.0)
        .into()
}

/// The picture, scaled to fit. Pointer positions are mapped back to the remote desktop.
fn desktop(live: &Live) -> Element<'_, Message> {
    let handle: Handle = live.frame.clone();
    let remote = live.size;
    responsive(move |area| {
        let picture = image(handle.clone())
            .content_fit(ContentFit::Contain)
            .width(Length::Fill)
            .height(Length::Fill);
        mouse_area(picture)
            .on_move(move |point| {
                let (x, y) = remote_position(area, remote, point);
                Message::Moved(x, y)
            })
            .on_press(Message::Button(Button::Left, true))
            .on_release(Message::Button(Button::Left, false))
            .on_right_press(Message::Button(Button::Right, true))
            .on_right_release(Message::Button(Button::Right, false))
            .on_middle_press(Message::Button(Button::Middle, true))
            .on_middle_release(Message::Button(Button::Middle, false))
            .on_scroll(|delta| Message::Wheel(wheel_units(delta)))
            .interaction(mouse::Interaction::None)
            .into()
    })
    .into()
}

/// Where a point of the view is on the remote desktop, when the picture is
/// scaled to fit the view and centred. Points outside the picture snap to its edge.
pub(crate) fn remote_position(view: Size, remote: (u32, u32), point: Point) -> (u16, u16) {
    let (width, height) = (remote.0.max(1) as f32, remote.1.max(1) as f32);
    let scale = (view.width / width).min(view.height / height);
    if scale <= 0.0 {
        return (0, 0);
    }
    let left = (view.width - width * scale) / 2.0;
    let top = (view.height - height * scale) / 2.0;
    let x = ((point.x - left) / scale).clamp(0.0, width - 1.0);
    let y = ((point.y - top) / scale).clamp(0.0, height - 1.0);
    (x as u16, y as u16)
}

/// One notch of a wheel is 120 units, like on Windows.
fn wheel_units(delta: mouse::ScrollDelta) -> i16 {
    match delta {
        mouse::ScrollDelta::Lines { y, .. } => (y * 120.0) as i16,
        // Touchpads report pixels; a few pixels per notch feels about right.
        mouse::ScrollDelta::Pixels { y, .. } => (y * 3.0) as i16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_that_fills_the_view_maps_one_to_one() {
        let at = remote_position(
            Size::new(1280.0, 800.0),
            (1280, 800),
            Point::new(100.0, 50.0),
        );

        assert_eq!(at, (100, 50));
    }

    #[test]
    fn a_larger_view_scales_and_centres_the_picture() {
        // 2x wide view with the same shape: scale 2, no bars.
        let scaled = remote_position(
            Size::new(2560.0, 1600.0),
            (1280, 800),
            Point::new(200.0, 100.0),
        );
        // A square view leaves bars above and below a 2:1 picture.
        let barred = remote_position(
            Size::new(1000.0, 1000.0),
            (1000, 500),
            Point::new(10.0, 250.0),
        );

        assert_eq!(scaled, (100, 50));
        assert_eq!(barred, (10, 0));
    }

    #[test]
    fn points_in_the_bars_snap_to_the_edge_of_the_picture() {
        let top = remote_position(
            Size::new(1000.0, 1000.0),
            (1000, 500),
            Point::new(500.0, 10.0),
        );
        let outside = remote_position(
            Size::new(1000.0, 1000.0),
            (1000, 500),
            Point::new(5000.0, 5000.0),
        );

        assert_eq!(top, (500, 0));
        assert_eq!(outside, (999, 499));
    }

    #[test]
    fn a_wheel_notch_is_120_units() {
        let notch = mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 };
        let down = mouse::ScrollDelta::Lines { x: 0.0, y: -2.0 };

        assert_eq!(wheel_units(notch), 120);
        assert_eq!(wheel_units(down), -240);
    }
}
