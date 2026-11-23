//! The shortcuts overlay (F1): a box over the window listing the keys the app understands.

use iced::widget::{column, container, mouse_area, opaque, row, text};
use iced::{Element, Length};

use crate::i18n::fl;
use crate::theme;

const WIDTH: f32 = 520.0;
/// How far down the window the box starts.
const TOP: f32 = 60.0;
const KEYS_WIDTH: f32 = 190.0;

#[derive(Debug, Clone)]
pub enum Message {
    Dismissed,
}

/// The keys and what they do, as `(keys, what)`.
pub fn lines() -> Vec<(String, String)> {
    vec![
        ("Ctrl+Shift+P".to_string(), fl!("shortcuts-palette")),
        ("Ctrl+Shift+T".to_string(), fl!("shortcuts-new-tab")),
        ("Ctrl+Shift+L".to_string(), fl!("shortcuts-local-terminal")),
        (
            "Ctrl+Shift+W, Ctrl+W".to_string(),
            fl!("shortcuts-close-tab"),
        ),
        (
            "Ctrl+Tab, Ctrl+Shift+Tab".to_string(),
            fl!("shortcuts-cycle"),
        ),
        ("Alt+1 … Alt+9".to_string(), fl!("shortcuts-number")),
        ("Ctrl+Shift+S".to_string(), fl!("shortcuts-split")),
        ("Ctrl+Shift+O".to_string(), fl!("shortcuts-other-pane")),
        ("Ctrl+Shift+R".to_string(), fl!("shortcuts-rotate")),
        (
            "Ctrl+Plus, Ctrl+Minus".to_string(),
            fl!("shortcuts-text-size"),
        ),
        ("Ctrl+F".to_string(), fl!("shortcuts-search")),
        ("↑ ↓ ← → Enter".to_string(), fl!("shortcuts-files-move")),
        ("Tab".to_string(), fl!("shortcuts-files-pane")),
        ("F2, Delete, F5".to_string(), fl!("shortcuts-files-act")),
        ("F1".to_string(), fl!("shortcuts-help")),
    ]
}

/// Covers the window so nothing behind it can be clicked while the overlay is open.
pub fn view<'a>() -> Element<'a, Message> {
    let rows = column(lines().into_iter().map(|(keys, what)| {
        row![
            container(text(keys).size(13)).width(Length::Fixed(KEYS_WIDTH)),
            text(what).style(theme::secondary_text),
        ]
        .spacing(theme::GAP)
        .into()
    }))
    .spacing(6);
    let dialog =
        container(column![text(fl!("shortcuts-title")).size(18), rows].spacing(theme::GAP))
            .width(Length::Fixed(WIDTH))
            .padding(theme::PANEL_PADDING)
            .style(theme::floating);
    let positioned = container(opaque(dialog))
        .center_x(Length::Fill)
        .padding([TOP, 0.0])
        .width(Length::Fill)
        .height(Length::Fill);
    opaque(mouse_area(positioned).on_press(Message::Dismissed))
}

#[cfg(test)]
#[path = "shortcuts_tests.rs"]
mod tests;
