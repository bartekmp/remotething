//! The strip under a remote desktop: a short summary, and an (i) button that
//! opens a panel of connection details. Shared by RDP and, later, VNC.

use iced::widget::{button, column, container, progress_bar, row, space, stack, text};
use iced::{Element, Length};

use crate::icons;
use crate::theme;

const HEIGHT: f32 = 30.0;
const PANEL_WIDTH: f32 = 360.0;

/// The most room the live text takes, so the buttons next to it keep theirs.
const LIVE_MAX: f32 = 260.0;

/// How thick the progress bar is.
const PROGRESS_GIRTH: f32 = 3.0;

/// What the bar shows.
pub(crate) struct Bar<Message> {
    /// Left side, for example the host and the desktop size.
    pub(crate) summary: String,
    /// Right side, for example the current bandwidth.
    pub(crate) live: String,
    /// While something is being recorded: shown in red, so it cannot be missed.
    pub(crate) recording: Option<String>,
    /// Label and value of every line in the details panel.
    pub(crate) details: Vec<(String, String)>,
    pub(crate) details_open: bool,
    /// How far a transfer has come, from 0 to 1, while one is going.
    pub(crate) progress: Option<f32>,
    /// Buttons left of the (i): their label and what pressing them does.
    pub(crate) actions: Vec<(String, Message)>,
}

/// `body` with the bar under it. The details float over the body's corner.
pub(crate) fn view<'a, Message: Clone + 'a>(
    body: Element<'a, Message>,
    bar: Bar<Message>,
    on_toggle: Message,
) -> Element<'a, Message> {
    let info = button(icons::view(icons::INFO, theme::TEXT_SMALL))
        .padding([2, 8])
        .style(theme::entry(bar.details_open))
        .on_press(on_toggle);
    let actions = bar.actions.into_iter().map(|(label, message)| {
        button(
            text(label)
                .size(theme::TEXT_SMALL)
                .wrapping(text::Wrapping::None),
        )
        .padding([2, 8])
        .style(theme::pill)
        .on_press(message)
        .into()
    });
    let strip = row![
        text(bar.summary)
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text)
            .wrapping(text::Wrapping::None),
        space().width(Length::Fill),
    ]
    .extend(bar.recording.map(|elapsed| {
        text(format!("● {elapsed}"))
            .size(theme::TEXT_SMALL)
            .style(text::danger)
            .into()
    }))
    // The live text gives way to the buttons when there is not room for both.
    .push(
        container(
            text(bar.live)
                .size(theme::TEXT_SMALL)
                .style(theme::secondary_text)
                .wrapping(text::Wrapping::None),
        )
        .width(Length::Shrink)
        .max_width(LIVE_MAX)
        .clip(true),
    )
    .extend(actions)
    .push(info)
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .height(Length::Fixed(HEIGHT));

    let mut page = column![body].width(Length::Fill).height(Length::Fill);
    if let Some(share) = bar.progress {
        page = page.push(progress_bar(0.0..=1.0, share.clamp(0.0, 1.0)).girth(PROGRESS_GIRTH));
    }
    let page = page.push(strip);
    if !bar.details_open {
        return page.into();
    }
    stack![page, details(bar.details)].into()
}

fn details<'a, Message: 'a>(lines: Vec<(String, String)>) -> Element<'a, Message> {
    let rows = lines.into_iter().map(|(label, value)| {
        row![
            text(label)
                .size(theme::TEXT_SMALL)
                .style(theme::secondary_text)
                .width(Length::FillPortion(2)),
            // Long values such as fingerprints have no spaces to break at.
            text(value)
                .size(theme::TEXT_SMALL)
                .wrapping(text::Wrapping::Glyph)
                .width(Length::FillPortion(3)),
        ]
        .spacing(8)
        .into()
    });
    let panel = container(column(rows).spacing(6))
        .width(Length::Fixed(PANEL_WIDTH))
        .padding(theme::PANEL_PADDING)
        .style(theme::floating);
    container(panel)
        .align_right(Length::Fill)
        .align_bottom(Length::Fill)
        .padding(iced::Padding::default().bottom(HEIGHT + 4.0))
        .into()
}
