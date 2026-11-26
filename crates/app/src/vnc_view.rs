//! Draws a VNC tab and turns the pointer over it into input.

use iced::widget::{button, column, container, mouse_area, responsive, sensor, text};
use iced::{Element, Length, mouse};
use remotething_vnc::Button;

use crate::credentials::Hidden;
use crate::desktop_cursor;
use crate::desktop_view::{remote_position, wheel_units};
use crate::features;
use crate::host_key_view;
use crate::i18n::fl;
use crate::meter;
use crate::note::Note;
use crate::picture;
use crate::quality_choice;
use crate::status_bar::{self, Bar};
use crate::theme;
use crate::vnc_tab::{Form, Live, Message, Stage, State};

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
        Stage::Asking(form) => centred(password_form(&state.connection.name, form)),
        Stage::HostKey { key, changed } => centred(host_key_view::question(
            key,
            *changed,
            Message::TrustPressed,
        )),
        Stage::Connected(live) => with_status_bar(
            live,
            &state.connection.host,
            route(state),
            state.connection.desktop_quality,
        ),
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

fn password_form<'a>(name: &str, form: &'a Form) -> Element<'a, Message> {
    let mut fields = column![text(name.to_string()).size(theme::TEXT_TITLE)]
        .spacing(10)
        .max_width(FORM_WIDTH);
    if let Some(note) = &form.note {
        fields = fields.push(text(note.as_str()).style(text::danger));
    }
    fields
        .push(
            theme::input(&fl!("session-password"), &form.password)
                .secure(true)
                .on_input(|typed| Message::PasswordChanged(Hidden(typed)))
                .on_submit(Message::Submitted),
        )
        .push(
            button(text(fl!("session-connect")))
                .style(theme::primary)
                .on_press(Message::Submitted),
        )
        .into()
}

/// How the connection reaches the server: its address, or the SSH servers it goes through.
fn route(state: &State) -> Option<String> {
    let hops: Vec<String> = state
        .jumps()
        .filter_map(|hop| hop.hop())
        .map(|hop| format!("{}:{}", hop.endpoint.host, hop.endpoint.port))
        .collect();
    (!hops.is_empty()).then(|| fl!("details-via", hosts = hops.join(" → ")))
}

fn with_status_bar<'a>(
    live: &'a Live,
    host: &str,
    via: Option<String>,
    quality: remotething_core::Quality,
) -> Element<'a, Message> {
    let mut actions = Vec::new();
    if features::screenshots() {
        actions.push((fl!("screenshot-button"), Message::ScreenshotPressed));
    }
    if features::desktop_recording() {
        let label = if live.video.is_active() {
            fl!("recording-stop")
        } else {
            fl!("recording-start")
        };
        actions.push((label, Message::RecordPressed));
    }
    let bar = Bar {
        summary: format!("{host} · {} × {}", live.size.0, live.size.1),
        live: live_rate(live),
        recording: live.video.elapsed().map(meter::clock),
        details: details(live, host, via, quality),
        details_open: live.details_open,
        progress: None,
        actions,
    };
    // The desktop follows the room it gets: its size is reported as the tab changes.
    let room = sensor(desktop(live))
        .on_show(Message::AreaChanged)
        .on_resize(Message::AreaChanged);
    status_bar::view(room.into(), bar, Message::DetailsToggled)
}

fn live_rate(live: &Live) -> String {
    if let Some(note) = live.note.as_ref().and_then(Note::current) {
        return note.to_string();
    }
    if let Some(problem) = live.video.problem() {
        return problem.to_string();
    }
    match &live.meter.reading {
        Some(reading) => format!(
            "↓ {}/s  ↑ {}/s · {} fps",
            meter::bytes(reading.received_per_second),
            meter::bytes(reading.sent_per_second),
            reading.frames_per_second
        ),
        None => String::new(),
    }
}

fn details(
    live: &Live,
    host: &str,
    via: Option<String>,
    quality: remotething_core::Quality,
) -> Vec<(String, String)> {
    let security = match via {
        Some(_) => fl!("details-security-tunnel"),
        None => live.info.security.to_string(),
    };
    let address = via
        .or_else(|| live.info.peer.map(|peer| peer.to_string()))
        .unwrap_or_default();
    let mut lines = vec![
        (fl!("details-protocol"), "VNC".to_string()),
        (fl!("details-host"), host.to_string()),
        (fl!("details-address"), address),
        (fl!("details-security"), security),
        (fl!("details-quality"), quality_choice::label(quality)),
        (fl!("details-encodings"), live.info.encodings.to_string()),
        (fl!("details-colours"), live.info.colours.to_string()),
        (
            fl!("details-size"),
            format!("{} × {}", live.size.0, live.size.1),
        ),
        (
            fl!("details-resizing"),
            if live.resizable {
                fl!("details-resizing-yes")
            } else {
                fl!("details-resizing-no")
            },
        ),
    ];
    if let Some(path) = live.video.path() {
        lines.push((fl!("details-recording"), path.display().to_string()));
    }
    if let Some(reading) = &live.meter.reading {
        lines.push((
            fl!("details-received"),
            format!(
                "{} ({}/s)",
                meter::bytes(reading.totals.received),
                meter::bytes(reading.received_per_second)
            ),
        ));
        lines.push((
            fl!("details-sent"),
            format!(
                "{} ({}/s)",
                meter::bytes(reading.totals.sent),
                meter::bytes(reading.sent_per_second)
            ),
        ));
        lines.push((fl!("details-frames"), reading.frames_per_second.to_string()));
        lines.push((fl!("details-uptime"), meter::clock(reading.uptime)));
    }
    lines
}

/// The picture, scaled to fit. Pointer positions are mapped back to the remote desktop.
fn desktop(live: &Live) -> Element<'_, Message> {
    let picture = live.picture.clone();
    let remote = live.size;
    let look = live.cursor.clone();
    let (pointer, over) = (live.pointer, live.over);
    let interaction = if look.hides_local() {
        mouse::Interaction::Hidden
    } else {
        mouse::Interaction::None
    };
    responsive(move |area| {
        let desktop = mouse_area(picture::view(&picture))
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
            .on_enter(Message::PointerOver(true))
            .on_exit(Message::PointerOver(false))
            .interaction(interaction);
        desktop_cursor::with_pointer(desktop.into(), &look, pointer, over, area, remote)
    })
    .into()
}
