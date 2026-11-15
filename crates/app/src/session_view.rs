//! How an SSH session looks in each of its stages.

use iced::widget::{button, column, container, opaque, row, stack, text};
use iced::{Element, Font, Length, Theme};
use remotething_remote::ssh::HostKey;

use crate::challenge;
use crate::credentials::Hidden;
use crate::files_pane;
use crate::files_view;
use crate::i18n::fl;
use crate::paste;
use crate::search;
use crate::session::{Form, Message, Stage, State};
use crate::session_strip;
use crate::status_bar;
use crate::terminal_view;
use crate::theme;

const FORM_WIDTH: f32 = 360.0;

pub fn view(state: &State) -> Element<'_, Message> {
    let host = state.connection.host.as_str();
    match &state.stage {
        Stage::LoadingSecret | Stage::Connecting => {
            centred(text(fl!("session-connecting", host = host)).style(theme::secondary_text))
        }
        Stage::Asking(form) => centred(sign_in_form(state, form)),
        Stage::Questions(form) => centred(challenge::view(&state.connection.name, form)),
        // The key may belong to a jump host on the way, so its own host is shown.
        Stage::HostKey { key, changed } => centred(host_key_question(key, *changed)),
        Stage::Connected(live) => {
            let terminal = terminal_view::view(&live.screen, &live.cache, Message::Terminal);
            let terminal = match &live.pending_paste {
                Some(pending) => stack![terminal, paste_question(pending)].into(),
                None => terminal,
            };
            let body: Element<'_, Message> = match &live.files {
                // The pane takes its own space beside the terminal; it does not cover it.
                Some(pane) => {
                    let pane = container(files_view::view(pane).map(Message::Files))
                        .width(Length::Fixed(files_pane::WIDTH))
                        .height(Length::Fill)
                        .padding(theme::GAP)
                        .style(theme::chip);
                    row![terminal, pane].spacing(theme::GAP).into()
                }
                None => {
                    let browse = button(text(fl!("files-browse")).size(theme::TEXT_SMALL))
                        .style(theme::pill)
                        .on_press(Message::BrowseFilesPressed);
                    let toolbar = container(browse).align_right(Length::Fill);
                    column![toolbar, terminal].spacing(6).into()
                }
            };
            let mut layers = column![].spacing(6);
            if let Some(bar) = live.forwards.view() {
                layers = layers.push(bar);
            }
            if let Some(open) = &live.search {
                layers = layers.push(search::view(open, live.screen.found.len()));
            }
            let content = layers.push(body).into();
            status_bar::view(
                content,
                session_strip::bar(state, live),
                Message::DetailsToggled,
            )
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

/// Asks before a long or multi-line paste goes to the shell. It covers the terminal.
fn paste_question(pending: &str) -> Element<'_, Message> {
    let size = fl!(
        "paste-size",
        lines = paste::line_count(pending),
        characters = pending.chars().count()
    );
    let preview = container(
        text(paste::preview(pending))
            .font(Font::MONOSPACE)
            .size(theme::TEXT_SMALL)
            .wrapping(text::Wrapping::Glyph),
    )
    .width(Length::Fill)
    .padding(8)
    .style(theme::chip);
    let buttons = row![
        button(text(fl!("paste-yes")))
            .style(theme::primary)
            .on_press(Message::PasteConfirmed),
        button(text(fl!("paste-no")))
            .style(theme::pill)
            .on_press(Message::PasteCancelled),
    ]
    .spacing(8);
    let question = column![
        text(fl!("paste-question")).size(theme::TEXT_TITLE),
        text(size).style(theme::secondary_text),
        preview,
        text(fl!("paste-explain"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        buttons,
    ]
    .spacing(theme::GAP);
    let dialog = container(question)
        .max_width(FORM_WIDTH + 120.0)
        .padding(theme::PANEL_PADDING)
        .style(theme::floating);
    opaque(container(dialog).center(Length::Fill))
}

fn centred<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center(Length::Fill).into()
}

fn sign_in_form<'a>(state: &State, form: &'a Form) -> Element<'a, Message> {
    let mut fields = column![text(state.connection.name.clone()).size(theme::TEXT_TITLE)]
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
    let secret_label = if state.asks_for_passphrase() {
        fl!("session-passphrase")
    } else {
        fl!("session-password")
    };
    fields = fields.push(
        theme::input(&secret_label, &form.secret)
            .secure(true)
            .on_input(|typed| Message::SecretChanged(Hidden(typed)))
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

fn host_key_question(key: &HostKey, changed: bool) -> Element<'_, Message> {
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
            .on_press(Message::TrustPressed),
    ]
    .spacing(8);

    column![explanation, fingerprint, buttons]
        .spacing(theme::GAP)
        .max_width(FORM_WIDTH + 120.0)
        .into()
}
