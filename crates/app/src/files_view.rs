//! How the file pane is drawn.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text, tooltip};
use iced::{Element, Font, Length};
use remotething_files::EntryKind;

use crate::file_details::{modified_text, permissions_text};
use crate::files_dual;
use crate::files_keys;
use crate::files_menu_view;
use crate::files_pane::{Listing, Message, Row, State, Status, format_size};
use crate::files_preview_view;
use crate::files_transfers_view;
use crate::i18n::fl;
use crate::icons;
use crate::theme;

const INDENT: f32 = 14.0;
const ARROW_WIDTH: f32 = 14.0;
/// Widths of the detail columns, in monospace characters of the small text.
const PERMISSIONS_WIDTH: f32 = 80.0;
const MODIFIED_WIDTH: f32 = 118.0;
const SIZE_WIDTH: f32 = 64.0;

/// The pane, with the local pane and the copy buttons beside it when that is showing.
pub fn view(state: &State) -> Element<'_, Message> {
    let Some(other) = &state.other else {
        return pane(state, None);
    };
    let copy = |label: String, message: Message, enabled: bool| {
        button(text(label).size(theme::TEXT_SMALL))
            .style(theme::pill)
            .on_press_maybe(enabled.then_some(message))
    };
    let buttons = column![
        copy(
            fl!("files-copy-to-local"),
            Message::CopyToLocalPressed,
            files_dual::can_copy_to_local(state)
        ),
        copy(
            fl!("files-copy-to-remote"),
            Message::CopyToRemotePressed,
            files_dual::can_copy_to_remote(state)
        ),
    ]
    .spacing(theme::GAP)
    .align_x(Horizontal::Center);
    row![
        container(pane(state, Some(!state.focus_other))).width(Length::FillPortion(1)),
        container(buttons).center_y(Length::Fill),
        container(pane(other, Some(state.focus_other)).map(files_dual::wrap))
            .width(Length::FillPortion(1)),
    ]
    .spacing(theme::GAP)
    .into()
}

/// One pane. With two showing, `focus` says whether this one has the keyboard.
fn pane(state: &State, focus: Option<bool>) -> Element<'_, Message> {
    let title = if state.is_local {
        fl!("files-title-local")
    } else {
        fl!("files-title")
    };
    let mut title = text(title).size(theme::TEXT_TITLE).width(Length::Fill);
    title = match focus {
        Some(true) => title.style(text::base),
        Some(false) => title.style(theme::secondary_text),
        None => title,
    };
    let mut header = row![title].align_y(Vertical::Center);
    if state.closable {
        let (icon, label) = if state.zoomed {
            (icons::MINIMIZE, fl!("files-unzoom"))
        } else {
            (icons::MAXIMIZE, fl!("files-zoom"))
        };
        let content: Element<'_, Message> = if state.is_compact() {
            icons::view(icon, theme::TEXT_BODY).into()
        } else {
            row![
                icons::view(icon, theme::TEXT_SMALL),
                text(label).size(theme::TEXT_SMALL)
            ]
            .spacing(6)
            .align_y(Vertical::Center)
            .into()
        };
        header = header.push(
            button(content)
                .style(theme::pill)
                .on_press(Message::ZoomToggled),
        );
        header = header.push(
            button(icons::view(icons::CLOSE, theme::TEXT_BODY))
                .padding(6)
                .style(theme::entry(false))
                .on_press(Message::ClosePressed),
        );
    }
    let path = text(state.root.as_str())
        .font(Font::MONOSPACE)
        .size(theme::TEXT_SMALL)
        .style(theme::secondary_text)
        .wrapping(text::Wrapping::Glyph);

    // Beside a terminal the pane is narrow, so the buttons are icons; filling the tab they
    // have labels too.
    let compact = state.is_compact();
    let pill = |icon: icons::Icon, label: String, message: Option<Message>| {
        let content: Element<'_, Message> = if compact {
            icons::view(icon, theme::TEXT_BODY).into()
        } else {
            row![
                icons::view(icon, theme::TEXT_SMALL),
                text(label.clone()).size(theme::TEXT_SMALL)
            ]
            .spacing(6)
            .align_y(Vertical::Center)
            .into()
        };
        let pill = button(content).style(theme::pill).on_press_maybe(message);
        if compact {
            tooltip(
                pill,
                text(label).size(theme::TEXT_SMALL),
                tooltip::Position::Bottom,
            )
            .style(theme::chip)
            .into()
        } else {
            Element::from(pill)
        }
    };
    let ready = state.files.is_some();
    let mut toolbar = row![
        pill(
            icons::ARROW_UP,
            fl!("files-up"),
            ready.then_some(Message::UpPressed)
        ),
        pill(
            icons::REFRESH,
            fl!("files-refresh"),
            ready.then_some(Message::RefreshPressed)
        ),
    ];
    if !state.is_local {
        toolbar = toolbar.push(pill(
            icons::UPLOAD,
            fl!("files-upload"),
            ready.then_some(Message::UploadPressed),
        ));
    }
    toolbar = toolbar.push(pill(
        icons::EYE,
        fl!("files-preview"),
        (ready && state.selected_file().is_some()).then_some(Message::PreviewPressed),
    ));
    toolbar = toolbar.push(pill(
        icons::LIST,
        if state.details {
            fl!("files-details-hide")
        } else {
            fl!("files-details-show")
        },
        Some(Message::DetailsToggled),
    ));
    if !state.is_local {
        toolbar = toolbar
            .push(pill(
                icons::FOLDER_UP,
                fl!("files-upload-folder"),
                ready.then_some(Message::UploadFolderPressed),
            ))
            .push(pill(
                icons::DOWNLOAD,
                fl!("files-download"),
                (ready && state.can_download()).then_some(Message::DownloadPressed),
            ));
        // A second pane has no room beside a terminal.
        if !compact {
            toolbar = toolbar.push(pill(
                icons::HARD_DRIVE,
                if state.other.is_some() {
                    fl!("files-local-hide")
                } else {
                    fl!("files-local-show")
                },
                ready.then_some(Message::LocalToggled),
            ));
        }
    }
    let toolbar = toolbar.spacing(6).wrap();

    let rows = state.rows();
    let list: Element<'_, Message> = match state.listings.get(&state.root) {
        Some(Listing::Failed(reason)) => text(reason.as_str()).style(text::danger).into(),
        Some(Listing::Loaded(_)) if rows.is_empty() => {
            text(fl!("files-empty")).style(theme::secondary_text).into()
        }
        Some(Listing::Loaded(_)) => {
            scrollable(column(rows.into_iter().map(|row| view_row(state, row))))
                .id(files_keys::list_id(state))
                .into()
        }
        _ if state.status.is_some() => space().into(),
        _ => text(fl!("files-loading"))
            .style(theme::secondary_text)
            .into(),
    };

    let status: Element<'_, Message> = match &state.pending_replace {
        Some((_, remote)) => replace_question(remote),
        None => view_status(&state.status),
    };

    let body: Element<'_, Message> = match &state.preview {
        Some(preview) => files_preview_view::view(preview),
        None => container(list).height(Length::Fill).into(),
    };
    let mut page = column![header, path, toolbar, body].spacing(theme::GAP);
    if let Some(transfers) = files_transfers_view::view(state) {
        page = page.push(transfers);
    }
    page.push(status).into()
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
    if state.details {
        let cell = |content: String, width: f32| {
            text(content)
                .font(Font::MONOSPACE)
                .size(theme::TEXT_SMALL)
                .style(theme::secondary_text)
                .width(Length::Fixed(width))
                .wrapping(text::Wrapping::None)
        };
        let modified = tree_row.entry.modified.map(modified_text);
        let permissions = tree_row.entry.permissions.map(permissions_text);
        line = line
            .push(cell(permissions.unwrap_or_default(), PERMISSIONS_WIDTH))
            .push(cell(modified.unwrap_or_default(), MODIFIED_WIDTH));
    }
    if let (false, Some(size)) = (is_folder, tree_row.entry.size) {
        line = line.push(
            text(format_size(size))
                .size(theme::TEXT_SMALL)
                .style(theme::secondary_text)
                .width(Length::Fixed(SIZE_WIDTH))
                .align_x(iced::alignment::Horizontal::Right),
        );
    } else if state.details {
        line = line.push(space::horizontal().width(Length::Fixed(SIZE_WIDTH)));
    }

    let kind = tree_row.entry.kind;
    let menu_here = state
        .menu
        .as_ref()
        .filter(|menu| menu.path == tree_row.path);
    let right_press = Message::MenuOpened(tree_row.path.clone(), kind);
    let on_press = if is_folder {
        Message::FolderToggled(tree_row.path)
    } else {
        Message::Selected(tree_row.path, kind)
    };
    let line = container(line)
        .padding([5, 8])
        .width(Length::Fill)
        .clip(true)
        .style(theme::list_row(is_selected));
    let line = mouse_area(line)
        .on_press(on_press)
        .on_right_press(right_press);
    match menu_here {
        Some(menu) => column![line, files_menu_view::view(menu)].into(),
        None => line.into(),
    }
}

/// The last problem, if there is one.
fn view_status(status: &Option<Status>) -> Element<'_, Message> {
    match status {
        Some(Status::Failed(reason)) => text(reason.as_str())
            .size(theme::TEXT_SMALL)
            .style(text::danger)
            .into(),
        None => space().into(),
    }
}

/// Asks before an upload replaces a file that is already there.
fn replace_question(remote: &str) -> Element<'_, Message> {
    let buttons = row![
        button(text(fl!("files-replace-yes")).size(theme::TEXT_SMALL))
            .style(theme::danger)
            .on_press(Message::ReplaceConfirmed),
        button(text(fl!("files-replace-no")).size(theme::TEXT_SMALL))
            .style(theme::pill)
            .on_press(Message::ReplaceCancelled),
    ]
    .spacing(6);
    column![
        text(fl!("files-replace-question", file = remote)).size(theme::TEXT_SMALL),
        buttons
    ]
    .spacing(6)
    .into()
}
