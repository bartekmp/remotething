//! The question asked before a folder with content is removed.

use iced::widget::{button, column, container, opaque, row, text};
use iced::{Element, Length};

use crate::i18n::fl;
use crate::theme;

/// A folder waiting for a yes or no.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub folder_id: String,
    name: String,
    folders: usize,
    connections: usize,
}

impl Question {
    pub fn new(folder_id: String, name: String, inside: (usize, usize)) -> Question {
        Question {
            folder_id,
            name,
            folders: inside.0,
            connections: inside.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Confirmed,
    Cancelled,
}

/// Covers the window so nothing else can be clicked while the question is open.
pub fn view(question: &Question) -> Element<'_, Message> {
    let buttons = row![
        button(text(fl!("removal-yes")))
            .style(theme::danger)
            .on_press(Message::Confirmed),
        button(text(fl!("removal-no")))
            .style(theme::pill)
            .on_press(Message::Cancelled),
    ]
    .spacing(8);
    let contents = fl!(
        "removal-contents",
        folders = question.folders,
        connections = question.connections
    );
    let body = column![
        text(fl!("removal-question", name = question.name.as_str())).size(theme::TEXT_TITLE),
        text(contents).style(theme::secondary_text),
        buttons,
    ]
    .spacing(theme::GAP);
    let dialog = container(body)
        .max_width(380.0)
        .padding(theme::PANEL_PADDING)
        .style(theme::floating);
    opaque(container(dialog).center(Length::Fill))
}
