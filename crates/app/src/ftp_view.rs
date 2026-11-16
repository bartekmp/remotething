//! Draws an FTP or FTPS tab.

use iced::widget::{button, column, container, row, text};
use iced::{Element, Font, Length, Theme};
use remotething_core::Protocol;

use crate::credentials::Hidden;
use crate::files_view;
use crate::ftp_tab::{Form, Message, Stage, State};
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
        Stage::Browsing(pane) => {
            let page = container(files_view::view(pane).map(Message::Files))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(theme::GAP)
                .style(theme::chip);
            column![page, strip(state)].spacing(6).into()
        }
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

/// Who is signed in where, and whether the connection is protected. Plain FTP says so in red.
fn strip(state: &State) -> Element<'_, Message> {
    let connection = &state.connection;
    let summary = format!("{}@{}:{}", state.username, connection.host, connection.port);
    let protection = match connection.protocol {
        Protocol::Ftps => text(fl!("ftp-encrypted"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        // SMB 2 and 3 sign their messages, and encrypt them when the server asks.
        Protocol::Smb => text(fl!("smb-label"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        _ => text(fl!("ftp-not-encrypted"))
            .size(theme::TEXT_SMALL)
            .style(text::danger),
    };
    row![
        text(summary)
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text)
            .width(Length::Fill),
        protection
    ]
    .into()
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
