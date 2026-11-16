//! How the menu of a row in the file pane is drawn: a small panel under the row.

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

use crate::files_menu::{Action, Menu, Mode, NAME_INPUT, actions, delete_question, is_valid_name};
use crate::files_pane::Message;
use crate::i18n::fl;
use crate::theme;

pub(crate) fn view(menu: &Menu) -> Element<'_, Message> {
    let content: Element<'_, Message> = match &menu.mode {
        Mode::Actions => column(actions(menu.kind).iter().map(|action| item(*action))).into(),
        Mode::Renaming(name) => name_field(fl!("files-rename-label"), name),
        Mode::NewFolder(name) => name_field(fl!("files-new-folder-label"), name),
        Mode::ConfirmDelete => column![
            text(delete_question(&menu.path, menu.kind)).size(theme::TEXT_SMALL),
            row![
                button(text(fl!("files-delete")).size(theme::TEXT_SMALL))
                    .style(theme::danger)
                    .on_press(Message::MenuSubmitted),
                cancel(),
            ]
            .spacing(6),
        ]
        .spacing(6)
        .into(),
    };
    container(content)
        .padding(6)
        .width(Length::Fill)
        .style(theme::floating)
        .into()
}

fn item<'a>(action: Action) -> Element<'a, Message> {
    let style = if action == Action::Delete {
        text::danger
    } else {
        text::default
    };
    button(text(label(action)).size(theme::TEXT_SMALL).style(style))
        .width(Length::Fill)
        .padding([4, 8])
        .style(theme::entry(false))
        .on_press(Message::MenuChosen(action))
        .into()
}

fn label(action: Action) -> String {
    match action {
        Action::Preview => fl!("files-preview"),
        Action::Download => fl!("files-download"),
        Action::CopyPath => fl!("files-copy-path"),
        Action::Rename => fl!("files-rename"),
        Action::NewFolder => fl!("files-new-folder"),
        Action::Delete => fl!("files-delete"),
    }
}

/// A text field for a name, with a confirm button that waits for a valid one.
fn name_field(label: String, typed: &str) -> Element<'_, Message> {
    let valid = is_valid_name(typed);
    let field = theme::input("", typed)
        .id(NAME_INPUT)
        .padding(4)
        .on_input(Message::MenuTyped)
        .on_submit_maybe(valid.then_some(Message::MenuSubmitted));
    let confirm = button(text(fl!("files-menu-ok")).size(theme::TEXT_SMALL))
        .style(theme::pill)
        .on_press_maybe(valid.then_some(Message::MenuSubmitted));
    column![
        text(label)
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        field,
        row![confirm, cancel()].spacing(6),
    ]
    .spacing(6)
    .into()
}

fn cancel<'a>() -> Element<'a, Message> {
    button(text(fl!("files-replace-no")).size(theme::TEXT_SMALL))
        .style(theme::pill)
        .on_press(Message::MenuDismissed)
        .into()
}
