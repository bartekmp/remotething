//! The question asked before a folder with content is removed.

use iced::widget::{button, column, container, opaque, row, text};
use iced::{Element, Length};

use crate::i18n::fl;
use crate::theme;

/// What is about to be removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// A folder, with how many folders and connections are inside it.
    Folder {
        id: String,
        inside: (usize, usize),
    },
    Connection(String),
    Credential(String),
}

/// Something waiting for a yes or no.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub subject: Subject,
    name: String,
}

impl Question {
    pub fn new(subject: Subject, name: String) -> Question {
        Question { subject, name }
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
    let name = question.name.as_str();
    let (title, note) = match &question.subject {
        Subject::Folder { inside: (0, 0), .. } => (fl!("removal-folder", name = name), None),
        Subject::Folder {
            inside: (folders, connections),
            ..
        } => {
            let (folders, connections): (usize, usize) = (*folders, *connections);
            (
                fl!("removal-folder", name = name),
                Some(fl!(
                    "removal-contents",
                    folders = folders,
                    connections = connections
                )),
            )
        }
        Subject::Connection(_) => (fl!("removal-connection", name = name), None),
        Subject::Credential(_) => (
            fl!("removal-credential", name = name),
            Some(fl!("removal-credential-note")),
        ),
    };
    let mut body = column![text(title).size(theme::TEXT_TITLE)].spacing(theme::GAP);
    if let Some(note) = note {
        body = body.push(text(note).style(theme::secondary_text));
    }
    let body = body.push(buttons);
    let dialog = container(body)
        .max_width(380.0)
        .padding(theme::PANEL_PADDING)
        .style(theme::floating);
    opaque(container(dialog).center(Length::Fill))
}
