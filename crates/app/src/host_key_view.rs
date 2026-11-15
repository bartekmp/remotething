//! The question shown when a server's key is new or has changed.

use iced::widget::{button, column, row, text};
use iced::{Element, Font, Theme};
use remotething_remote::ssh::HostKey;

use crate::i18n::fl;
use crate::theme;

/// Width the question may take, matching the sign-in forms.
const WIDTH: f32 = 480.0;

/// Shows the key's fingerprint and a button that sends `on_trust`.
pub(crate) fn question<'a, Message: Clone + 'a>(
    key: &'a HostKey,
    changed: bool,
    on_trust: Message,
) -> Element<'a, Message> {
    let host = key.endpoint.host.as_str();
    let (explanation, trust_label) = if changed {
        (
            text(fl!("session-host-key-changed", host = host)).style(text::danger),
            fl!("session-trust-changed"),
        )
    } else {
        (
            text(fl!("session-host-key-new", host = host)),
            fl!("session-trust"),
        )
    };
    let fingerprint = column![
        text(fl!("session-fingerprint"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        // A fingerprint has no spaces to break at.
        text(key.fingerprint.as_str())
            .font(Font::MONOSPACE)
            .wrapping(text::Wrapping::Glyph),
    ]
    .spacing(2);
    // A changed key is the dangerous case, so trusting it is not the highlighted action.
    let trust_style: fn(&Theme, button::Status) -> button::Style = if changed {
        theme::danger
    } else {
        theme::primary
    };
    let buttons = row![
        button(text(trust_label))
            .style(trust_style)
            .on_press(on_trust)
    ]
    .spacing(8);

    column![explanation, fingerprint, buttons]
        .spacing(theme::GAP)
        .max_width(WIDTH)
        .into()
}
