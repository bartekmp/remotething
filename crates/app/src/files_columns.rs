//! The detail columns of a file pane: their widths, and the header whose edges are dragged.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use iced::widget::{container, mouse_area, row, space, text};
use iced::{Element, Length};

use crate::files_pane::Message;
use crate::i18n::fl;
use crate::theme;

const MIN_WIDTH: f32 = 40.0;
const MAX_WIDTH: f32 = 400.0;
/// The strip between two columns that is grabbed.
const HANDLE_WIDTH: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Column {
    Permissions,
    Modified,
    Size,
}

/// How wide the detail columns are. The name takes the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Widths {
    pub(crate) permissions: f32,
    pub(crate) modified: f32,
    pub(crate) size: f32,
}

/// The widths the next file pane starts with: the saved ones, then whatever was dragged last.
static SAVED: Mutex<Widths> = Mutex::new(Widths::STANDARD);
/// Set when the saved widths changed and the settings file has not heard of it yet.
static CHANGED: AtomicBool = AtomicBool::new(false);

impl Widths {
    const STANDARD: Widths = Widths {
        permissions: 80.0,
        modified: 118.0,
        size: 64.0,
    };

    /// The widths a new file pane starts with.
    pub(crate) fn saved() -> Self {
        *SAVED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Makes `widths` what new panes start with, such as the ones read from the settings.
    pub(crate) fn set_saved(widths: Widths) {
        *SAVED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = widths;
    }

    /// Remembers the widths of a pane whose column edge was just let go of.
    pub(crate) fn settle(self) {
        if self != Self::saved() {
            Self::set_saved(self);
            CHANGED.store(true, Ordering::Relaxed);
        }
    }

    /// True once after the widths were changed by dragging, for the settings to be saved.
    pub(crate) fn take_changed() -> bool {
        CHANGED.swap(false, Ordering::Relaxed)
    }

    fn of(&mut self, column: Column) -> &mut f32 {
        match column {
            Column::Permissions => &mut self.permissions,
            Column::Modified => &mut self.modified,
            Column::Size => &mut self.size,
        }
    }

    /// Moves the left edge of `column` by `delta` pixels; the column is on the right, so
    /// dragging left makes it wider.
    pub(crate) fn drag(&mut self, column: Column, delta: f32) {
        let width = self.of(column);
        *width = (*width - delta).clamp(MIN_WIDTH, MAX_WIDTH);
    }
}

/// A column edge being dragged, and where the pointer was last seen across the header.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Drag {
    pub(crate) column: Column,
    pub(crate) last_x: Option<f32>,
}

/// The pointer moved across the header to `x`: widens or narrows the dragged column.
pub(crate) fn pointer_moved(widths: &mut Widths, drag: &mut Option<Drag>, x: f32) {
    let Some(drag) = drag else {
        return;
    };
    if let Some(last) = drag.last_x {
        widths.drag(drag.column, x - last);
    }
    drag.last_x = Some(x);
}

/// The header above the rows: the column names, with a handle at the left edge of each.
pub(crate) fn header<'a>(widths: &Widths) -> Element<'a, Message> {
    let cell = |column: Column, label: String, width: f32| {
        row![
            handle(column),
            text(label)
                .size(theme::TEXT_SMALL)
                .style(theme::secondary_text)
                .width(Length::Fixed(width - HANDLE_WIDTH))
                .wrapping(text::Wrapping::None),
        ]
    };
    let line = row![
        space::horizontal().width(Length::Fill),
        cell(
            Column::Permissions,
            fl!("files-column-permissions"),
            widths.permissions
        ),
        cell(
            Column::Modified,
            fl!("files-column-modified"),
            widths.modified
        ),
        cell(Column::Size, fl!("files-column-size"), widths.size),
    ]
    .spacing(6);
    // Row cells have 8px of padding and a 6px gap; the header lines up with them.
    mouse_area(container(line).padding([0, 8]))
        .on_move(|point| Message::ColumnMoved(point.x))
        .on_release(Message::ColumnReleased)
        .on_exit(Message::ColumnReleased)
        .into()
}

fn handle<'a>(column: Column) -> Element<'a, Message> {
    mouse_area(
        container(space::horizontal())
            .width(Length::Fixed(HANDLE_WIDTH))
            .height(Length::Fixed(16.0)),
    )
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .on_press(Message::ColumnPressed(column))
    .into()
}
