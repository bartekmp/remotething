//! How the file pane is drawn.

use iced::alignment::Vertical;
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text};
use iced::{Element, Font, Length};
use remotething_files::EntryKind;

use crate::files_pane::{Listing, Message, Row, State, Status, format_size};
use crate::i18n::fl;
use crate::icons;
use crate::theme;

const INDENT: f32 = 14.0;
const ARROW_WIDTH: f32 = 14.0;

pub fn view(state: &State) -> Element<'_, Message> {
    let header = row![
        text(fl!("files-title"))
            .size(theme::TEXT_TITLE)
            .width(Length::Fill),
        button(icons::view(icons::CLOSE, theme::TEXT_BODY))
            .padding(6)
            .style(theme::entry(false))
            .on_press(Message::ClosePressed),
    ]
    .align_y(Vertical::Center);
    let path = text(state.root.as_str())
        .font(Font::MONOSPACE)
        .size(theme::TEXT_SMALL)
        .style(theme::secondary_text)
        .wrapping(text::Wrapping::Glyph);

    let pill = |label: String, message: Option<Message>| {
        button(text(label).size(theme::TEXT_SMALL))
            .style(theme::pill)
            .on_press_maybe(message)
    };
    let ready = state.files.is_some() && !state.busy;
    let toolbar = row![
        pill(fl!("files-up"), ready.then_some(Message::UpPressed)),
        pill(
            fl!("files-refresh"),
            ready.then_some(Message::RefreshPressed)
        ),
        pill(fl!("files-upload"), ready.then_some(Message::UploadPressed)),
        pill(
            fl!("files-download"),
            (ready && state.selected_file().is_some()).then_some(Message::DownloadPressed)
        ),
    ]
    .spacing(6)
    .wrap();

    let rows = state.rows();
    let list: Element<'_, Message> = match state.listings.get(&state.root) {
        Some(Listing::Failed(reason)) => text(reason.as_str()).style(text::danger).into(),
        Some(Listing::Loaded(_)) if rows.is_empty() => {
            text(fl!("files-empty")).style(theme::secondary_text).into()
        }
        Some(Listing::Loaded(_)) => {
            scrollable(column(rows.into_iter().map(|row| view_row(state, row)))).into()
        }
        _ if state.status.is_some() => space().into(),
        _ => text(fl!("files-loading"))
            .style(theme::secondary_text)
            .into(),
    };

    let status: Element<'_, Message> = match (&state.status, state.busy) {
        (_, true) => text(fl!("files-working"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text)
            .into(),
        (Some(Status::Failed(reason)), _) => text(reason.as_str())
            .size(theme::TEXT_SMALL)
            .style(text::danger)
            .into(),
        (Some(Status::Done(done)), _) => text(done.as_str()).size(theme::TEXT_SMALL).into(),
        (None, false) => space().into(),
    };

    column![
        header,
        path,
        toolbar,
        container(list).height(Length::Fill),
        status
    ]
    .spacing(theme::GAP)
    .into()
}

fn view_row(state: &State, tree_row: Row) -> Element<'_, Message> {
    let is_selected = state
        .selected
        .as_ref()
        .is_some_and(|(path, _)| *path == tree_row.path);
    let is_folder = tree_row.entry.kind == EntryKind::Folder;
    let indent = space::horizontal().width(Length::Fixed(INDENT * tree_row.depth as f32));

    let arrow: Element<'_, Message> = if is_folder {
        let arrow = if state.expanded.contains(&tree_row.path) {
            icons::CHEVRON_DOWN
        } else {
            icons::CHEVRON_RIGHT
        };
        icons::view(arrow, ARROW_WIDTH)
            .style(theme::secondary_text)
            .into()
    } else {
        space::horizontal().width(Length::Fixed(ARROW_WIDTH)).into()
    };
    let icon = if is_folder {
        icons::for_folder(None)
    } else {
        icons::FILE
    };
    let mut line = row![
        indent,
        arrow,
        icons::view(icon, 15),
        text(tree_row.entry.name.clone())
            .size(theme::TEXT_BODY)
            .wrapping(text::Wrapping::None)
            .width(Length::Fill),
    ]
    .spacing(6)
    .align_y(Vertical::Center);
    if let (false, Some(size)) = (is_folder, tree_row.entry.size) {
        line = line.push(
            text(format_size(size))
                .size(theme::TEXT_SMALL)
                .style(theme::secondary_text),
        );
    }

    let on_press = if is_folder {
        Message::FolderToggled(tree_row.path)
    } else {
        Message::Selected(tree_row.path, tree_row.entry.kind)
    };
    let line = container(line)
        .padding([5, 8])
        .width(Length::Fill)
        .clip(true)
        .style(theme::list_row(is_selected));
    mouse_area(line).on_press(on_press).into()
}
