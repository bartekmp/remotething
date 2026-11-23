//! The command palette: a box over the window to jump to a connection or an
//! action by typing a few letters, without the mouse.

use iced::alignment::Vertical;
use iced::widget::{button, column, container, mouse_area, opaque, row, text};
use iced::{Element, Length};
use remotething_core::Store;

use crate::i18n::fl;
use crate::icons::{self, Icon};
use crate::launcher;
use crate::theme;
use crate::tree::protocol_label;

/// The widget id of the palette's field, so it can be focused when the palette opens.
pub const INPUT: &str = "palette-input";
const WIDTH: f32 = 480.0;
/// How far down the window the box starts.
const TOP: f32 = 80.0;

/// What a line of the palette does when chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// Opens the connection with this id.
    Connection(String),
    NewTab,
    LocalTerminal,
    Credentials,
    Settings,
}

/// One line of the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub item: Item,
    pub label: String,
    /// The host of a connection, or nothing for an action.
    pub hint: String,
    /// The protocol of a connection, or nothing for an action.
    pub badge: Option<String>,
    pub icon: Icon,
}

#[derive(Debug, Clone, Default)]
pub struct State {
    query: String,
    /// The highlighted line, an index into the entries.
    selected: usize,
}

#[derive(Debug, Clone)]
pub enum Message {
    Typed(String),
    /// Enter in the field.
    Submitted,
    /// A click on a line.
    Picked(usize),
    /// A click outside the box.
    Dismissed,
}

/// What the palette asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Chosen(Item),
    Close,
}

impl State {
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Moves the highlight, wrapping round the ends of the `count` entries.
    pub fn step(&mut self, forward: bool, count: usize) {
        if count == 0 {
            return;
        }
        self.selected = if forward {
            (self.selected + 1) % count
        } else {
            (self.selected + count - 1) % count
        };
    }
}

/// The lines to show: the connections that match, recent ones first, then the actions that match.
pub fn entries(store: &Store, recent: &[String], query: &str) -> Vec<Entry> {
    let mut found: Vec<Entry> = launcher::entries(store, recent, query)
        .into_iter()
        .map(|connection| Entry {
            item: Item::Connection(connection.id.clone()),
            label: connection.name.clone(),
            hint: connection.host.clone(),
            badge: Some(protocol_label(connection.protocol)),
            icon: icons::for_connection(connection.icon.as_deref(), connection.protocol),
        })
        .collect();
    let query = query.trim().to_lowercase();
    let actions = [
        (Item::NewTab, fl!("palette-new-tab"), icons::PLUS),
        (
            Item::LocalTerminal,
            fl!("palette-local-terminal"),
            icons::SQUARE_TERMINAL,
        ),
        (Item::Credentials, fl!("palette-credentials"), icons::KEY),
        (Item::Settings, fl!("palette-settings"), icons::SETTINGS),
    ];
    for (item, label, icon) in actions {
        if label.to_lowercase().contains(&query) {
            found.push(Entry {
                item,
                label,
                hint: String::new(),
                badge: None,
                icon,
            });
        }
    }
    found
}

pub fn update(state: &mut State, message: Message, entries: &[Entry]) -> Option<Event> {
    match message {
        Message::Typed(query) => {
            state.query = query;
            state.selected = 0;
            None
        }
        Message::Submitted => entries
            .get(state.selected)
            .map(|entry| Event::Chosen(entry.item.clone())),
        Message::Picked(index) => entries
            .get(index)
            .map(|entry| Event::Chosen(entry.item.clone())),
        Message::Dismissed => Some(Event::Close),
    }
}

/// Covers the window so nothing behind it can be clicked while the palette is open.
pub fn view(state: &State, entries: Vec<Entry>) -> Element<'_, Message> {
    let field = theme::input(&fl!("palette-search"), &state.query)
        .id(INPUT)
        .on_input(Message::Typed)
        .on_submit(Message::Submitted);
    let lines: Element<'_, Message> = if entries.is_empty() {
        text(fl!("palette-nothing"))
            .style(theme::secondary_text)
            .into()
    } else {
        column(
            entries
                .into_iter()
                .enumerate()
                .map(|(index, entry)| line(entry, index, index == state.selected)),
        )
        .spacing(2)
        .into()
    };
    let dialog = container(column![field, lines].spacing(theme::GAP))
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

fn line<'a>(entry: Entry, index: usize, highlighted: bool) -> Element<'a, Message> {
    let mut content = row![
        icons::view(entry.icon, 16),
        text(entry.label),
        text(entry.hint)
            .style(theme::secondary_text)
            .width(Length::Fill),
    ]
    .spacing(10)
    .align_y(Vertical::Center);
    if let Some(badge) = entry.badge {
        content = content.push(
            container(text(badge).size(11))
                .padding([2, 6])
                .style(theme::chip),
        );
    }
    button(content)
        .width(Length::Fill)
        .padding([8, 10])
        .style(theme::entry(highlighted))
        .on_press(Message::Picked(index))
        .into()
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
