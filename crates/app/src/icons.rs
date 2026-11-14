//! Line icons from the bundled Lucide icon font (ISC licence).

use iced::widget::{Text, button, row, text};
use iced::{Element, Font, Pixels};
use remotething_core::Protocol;

use crate::theme;

pub const FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/lucide.ttf");
const FONT: Font = Font::with_name("lucide");

/// One glyph of the icon font, with the name used in the connections file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon {
    pub name: &'static str,
    glyph: char,
}

const fn icon(name: &'static str, glyph: char) -> Icon {
    Icon { name, glyph }
}

/// The icons a user can give to a folder or connection, in the order shown.
pub const CHOICES: [Icon; 22] = [
    icon("folder", '\u{e0dc}'),
    icon("server", '\u{e156}'),
    icon("database", '\u{e0b1}'),
    icon("monitor", '\u{e120}'),
    icon("terminal", '\u{e184}'),
    icon("laptop", '\u{e1cc}'),
    icon("cloud", '\u{e08c}'),
    icon("globe", '\u{e0eb}'),
    icon("house", '\u{e0f8}'),
    icon("building", '\u{e28f}'),
    icon("briefcase", '\u{e066}'),
    icon("network", '\u{e128}'),
    icon("router", '\u{e3c2}'),
    icon("cpu", '\u{e0ad}'),
    icon("hard-drive", '\u{e0f0}'),
    icon("box", '\u{e065}'),
    icon("lock", '\u{e10e}'),
    icon("shield", '\u{e15b}'),
    icon("key", '\u{e4a7}'),
    icon("wrench", '\u{e1b0}'),
    icon("flask", '\u{e0da}'),
    icon("star", '\u{e179}'),
];

// Icons used by the window itself.
pub const CHEVRON_DOWN: Icon = icon("chevron-down", '\u{e071}');
pub const CHEVRON_RIGHT: Icon = icon("chevron-right", '\u{e073}');
pub const CLOSE: Icon = icon("x", '\u{e1b1}');
pub const SETTINGS: Icon = icon("settings", '\u{e157}');
pub const FILE: Icon = icon("file", '\u{e0c4}');
pub const PLUS: Icon = icon("plus", '\u{e140}');
pub const PANEL_LEFT_CLOSE: Icon = icon("panel-left-close", '\u{e21b}');
pub const PANEL_LEFT_OPEN: Icon = icon("panel-left-open", '\u{e21c}');

/// The choice with this name, if there is one.
pub fn by_name(name: &str) -> Option<Icon> {
    CHOICES.iter().copied().find(|choice| choice.name == name)
}

/// A folder's own icon, or the folder icon when none or an unknown one is set.
pub fn for_folder(chosen: Option<&str>) -> Icon {
    chosen.and_then(by_name).unwrap_or(CHOICES[0])
}

/// A connection's own icon, or one that fits its protocol.
pub fn for_connection(chosen: Option<&str>, protocol: Protocol) -> Icon {
    if let Some(icon) = chosen.and_then(by_name) {
        return icon;
    }
    let default = match protocol {
        Protocol::Ssh => "terminal",
        Protocol::Rdp | Protocol::Vnc => "monitor",
    };
    by_name(default).unwrap_or(CHOICES[0])
}

/// The icon as a widget. It takes its colour from the text around it.
pub fn view<'a>(icon: Icon, size: impl Into<Pixels>) -> Text<'a> {
    text(icon.glyph.to_string()).font(FONT).size(size)
}

const GRID_ICON_SIZE: f32 = 18.0;

/// Every choice as a small button. The first one is `default_icon` and picks `None`.
pub fn grid<'a, Message: Clone + 'a>(
    chosen: Option<&str>,
    default_icon: Icon,
    on_pick: fn(Option<&'static str>) -> Message,
) -> Element<'a, Message> {
    let cell = |icon: Icon, is_chosen: bool, name: Option<&'static str>| {
        button(view(icon, GRID_ICON_SIZE))
            .padding(6)
            .style(theme::entry(is_chosen))
            .on_press(on_pick(name))
            .into()
    };
    let default_cell = cell(default_icon, chosen.is_none(), None);
    let cells = CHOICES
        .iter()
        .map(|choice| cell(*choice, chosen == Some(choice.name), Some(choice.name)));

    row(std::iter::once(default_cell).chain(cells))
        .spacing(2)
        .wrap()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_choice_has_its_own_name_and_glyph() {
        for (index, first) in CHOICES.iter().enumerate() {
            for second in &CHOICES[index + 1..] {
                assert_ne!(first.name, second.name);
                assert_ne!(first.glyph, second.glyph);
            }
        }
    }

    #[test]
    fn a_chosen_icon_wins_over_the_default() {
        assert_eq!(for_folder(Some("star")).name, "star");
        assert_eq!(
            for_connection(Some("database"), Protocol::Ssh).name,
            "database"
        );
    }

    #[test]
    fn defaults_follow_the_kind_of_item() {
        assert_eq!(for_folder(None).name, "folder");
        assert_eq!(for_connection(None, Protocol::Ssh).name, "terminal");
        assert_eq!(for_connection(None, Protocol::Rdp).name, "monitor");
        assert_eq!(for_connection(None, Protocol::Vnc).name, "monitor");
    }

    #[test]
    fn an_unknown_name_falls_back_to_the_default() {
        assert_eq!(for_folder(Some("no-such-icon")).name, "folder");
        assert_eq!(
            for_connection(Some("no-such-icon"), Protocol::Ssh).name,
            "terminal"
        );
    }
}
