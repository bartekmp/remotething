//! How the preview of a remote file is drawn, in place of the tree.

use iced::alignment::Vertical;
use iced::widget::{button, column, image, row, scrollable, text};
use iced::{ContentFit, Element, Font, Length};

use crate::file_preview::{Content, TEXT_LIMIT};
use crate::files_pane::{Message, format_size};
use crate::files_preview::Preview;
use crate::i18n::fl;
use crate::theme;

pub(crate) fn view(preview: &Preview) -> Element<'_, Message> {
    let header = row![
        text(preview.name.as_str())
            .size(theme::TEXT_BODY)
            .wrapping(text::Wrapping::None)
            .width(Length::Fill),
        button(text(fl!("files-preview-back")).size(theme::TEXT_SMALL))
            .style(theme::pill)
            .on_press(Message::PreviewClosed),
    ]
    .spacing(6)
    .align_y(Vertical::Center);

    let body: Element<'_, Message> = match &preview.content {
        None => note(fl!("files-loading")),
        Some(Err(reason)) => text(reason.as_str())
            .size(theme::TEXT_SMALL)
            .style(text::danger)
            .into(),
        Some(Ok(Content::Text { text: shown, cut })) => {
            let mut page = column![
                text(shown.as_str())
                    .font(Font::MONOSPACE)
                    .size(theme::TEXT_SMALL)
                    .wrapping(text::Wrapping::Glyph)
            ]
            .spacing(theme::GAP);
            if *cut {
                page = page.push(note(fl!(
                    "files-preview-cut",
                    size = format_size(TEXT_LIMIT)
                )));
            }
            scrollable(page)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
        Some(Ok(Content::Picture(handle))) => scrollable(
            image(handle.clone())
                .width(Length::Fill)
                .content_fit(ContentFit::ScaleDown),
        )
        .height(Length::Fill)
        .into(),
        Some(Ok(Content::TooLarge)) => note(fl!("files-preview-too-large")),
        Some(Ok(Content::Binary)) => note(fl!("files-preview-binary")),
        Some(Ok(Content::Broken(reason))) => {
            note(fl!("files-preview-broken", reason = reason.as_str()))
        }
    };
    column![header, body]
        .spacing(theme::GAP)
        .height(Length::Fill)
        .into()
}

fn note(label: String) -> Element<'static, Message> {
    text(label)
        .size(theme::TEXT_SMALL)
        .style(theme::secondary_text)
        .into()
}
