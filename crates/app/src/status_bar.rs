//! The strip under a remote desktop: a short summary, and an (i) button that
//! opens a panel of connection details. Shared by RDP and, later, VNC.

use iced::widget::{button, column, container, row, space, stack, text};
use iced::{Element, Length};

use crate::icons;
use crate::theme;

const HEIGHT: f32 = 30.0;
const PANEL_WIDTH: f32 = 320.0;

/// What the bar shows.
pub(crate) struct Bar<Message> {
    /// Left side, for example the host and the desktop size.
    pub(crate) summary: String,
    /// Right side, for example the current bandwidth.
    pub(crate) live: String,
    /// Label and value of every line in the details panel.
    pub(crate) details: Vec<(String, String)>,
    pub(crate) details_open: bool,
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
        button(text(label).size(theme::TEXT_SMALL))
            .padding([2, 8])
            .style(theme::pill)
            .on_press(message)
            .into()
    });
    let strip = row![
        text(bar.summary)
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        space().width(Length::Fill),
        text(bar.live)
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
    ]
    .extend(actions)
    .push(info)
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .height(Length::Fixed(HEIGHT));

    let page = column![body, strip]
        .width(Length::Fill)
        .height(Length::Fill);
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
            text(value)
                .size(theme::TEXT_SMALL)
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
