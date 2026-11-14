//! The page of a new tab: recent connections, and a search over all of them.

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};
use remotething_core::{Connection, QuickTarget, Store, parse_quick};

use crate::i18n::fl;
use crate::icons;
use crate::theme;
use crate::tree::protocol_label;

/// The widget id of the search field, so a new tab can start with the keyboard in it.
pub const SEARCH_INPUT: &str = "launcher-search";
const WIDTH: f32 = 460.0;
/// More matches than this are not listed; the search has to be narrowed.
const LIMIT: usize = 30;

#[derive(Debug, Clone)]
pub enum Message {
    QueryChanged(String),
    Picked(String),
    /// Connect to what the typed connection string names, without saving it.
    QuickPicked(QuickTarget),
    /// Enter in the search field: connect to a typed connection string, or open the first entry.
    Submitted,
}

/// The connections to offer, best first.
///
/// Without a search these are the recent ones, newest first. With one, every
/// connection whose name or host contains it, recent ones before the others.
pub fn entries<'a>(store: &'a Store, recent: &[String], query: &str) -> Vec<&'a Connection> {
    let query = query.trim().to_lowercase();
    let matches = |connection: &Connection| {
        connection.name.to_lowercase().contains(&query)
            || connection.host.to_lowercase().contains(&query)
    };
    let find = |id: &String| store.connections.iter().find(|c| &c.id == id);

    // A recent connection may have been removed since.
    let mut found: Vec<&Connection> = recent
        .iter()
        .filter_map(find)
        .filter(|connection| matches(connection))
        .collect();
    if !query.is_empty() {
        let others = store
            .connections
            .iter()
            .filter(|connection| matches(connection) && !recent.contains(&connection.id));
        found.extend(others);
    }
    found.truncate(LIMIT);
    found
}

pub fn view<'a>(store: &'a Store, recent: &[String], query: &'a str) -> Element<'a, Message> {
    let search = theme::input(&fl!("launcher-search"), query)
        .id(SEARCH_INPUT)
        .on_input(Message::QueryChanged)
        .on_submit(Message::Submitted);
    let heading = if parse_quick(query).is_some() {
        fl!("launcher-quick-heading")
    } else if query.trim().is_empty() {
        fl!("launcher-recent")
    } else {
        fl!("launcher-matches")
    };

    let quick = parse_quick(query).map(quick_entry);
    let found = entries(store, recent, query);
    let list: Element<'a, Message> = if let Some(quick) = quick {
        // A connection string is not a search, so nothing else is offered.
        quick
    } else if found.is_empty() {
        let hint = if query.trim().is_empty() {
            fl!("launcher-no-recent")
        } else {
            fl!("launcher-no-matches")
        };
        text(hint).style(theme::secondary_text).into()
    } else {
        scrollable(column(found.into_iter().map(entry)).spacing(2)).into()
    };

    let content = column![
        search,
        text(heading)
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        list
    ]
    .spacing(theme::GAP)
    .width(Length::Fixed(WIDTH));
    container(content)
        .center_x(Length::Fill)
        .padding([40, 0])
        .into()
}

/// The one entry shown for a typed connection string.
fn quick_entry<'a>(target: QuickTarget) -> Element<'a, Message> {
    let icon = icons::for_connection(None, target.protocol);
    let badge = container(text(protocol_label(target.protocol)).size(11))
        .padding([2, 6])
        .style(theme::chip);
    let line = row![
        icons::view(icon, 16),
        text(fl!("launcher-quick", target = target.label())),
        text(fl!("launcher-quick-note"))
            .style(theme::secondary_text)
            .width(Length::Fill),
        badge,
    ]
    .spacing(10)
    .align_y(Vertical::Center);
    button(line)
        .width(Length::Fill)
        .padding([8, 10])
        .style(theme::entry(true))
        .on_press(Message::QuickPicked(target))
        .into()
}

fn entry(connection: &Connection) -> Element<'_, Message> {
    let icon = icons::for_connection(connection.icon.as_deref(), connection.protocol);
    let badge = container(text(protocol_label(connection.protocol)).size(11))
        .padding([2, 6])
        .style(theme::chip);
    let line = row![
        icons::view(icon, 16),
        text(connection.name.as_str()),
        text(connection.host.as_str())
            .style(theme::secondary_text)
            .width(Length::Fill),
        badge,
    ]
    .spacing(10)
    .align_y(Vertical::Center);
    button(line)
        .width(Length::Fill)
        .padding([8, 10])
        .style(theme::entry(false))
        .on_press(Message::Picked(connection.id.clone()))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let text = r#"
            [[connection]]
            id = "web-01"
            name = "web-01"
            protocol = "ssh"
            host = "web-01.example.net"

            [[connection]]
            id = "db-01"
            name = "Database"
            protocol = "rdp"
            host = "db-01.example.net"

            [[connection]]
            id = "laptop"
            name = "laptop"
            protocol = "vnc"
            host = "laptop.lan"
        "#;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        std::fs::write(&path, text).unwrap();
        Store::load(&path).unwrap()
    }

    fn ids(found: Vec<&Connection>) -> Vec<&str> {
        found.into_iter().map(|c| c.id.as_str()).collect()
    }

    fn recent(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn without_a_search_only_recent_connections_are_offered_newest_first() {
        let store = store();

        let found = entries(&store, &recent(&["laptop", "web-01"]), "");

        assert_eq!(ids(found), ["laptop", "web-01"]);
    }

    #[test]
    fn a_recent_connection_that_no_longer_exists_is_left_out() {
        let store = store();

        let found = entries(&store, &recent(&["gone", "web-01"]), "");

        assert_eq!(ids(found), ["web-01"]);
    }

    #[test]
    fn a_search_covers_every_connection_by_name_or_host() {
        let store = store();

        assert_eq!(ids(entries(&store, &[], "DATA")), ["db-01"]);
        assert_eq!(ids(entries(&store, &[], ".lan")), ["laptop"]);
    }

    #[test]
    fn recent_matches_come_before_the_others() {
        let store = store();

        let found = entries(&store, &recent(&["db-01"]), "01");

        assert_eq!(ids(found), ["db-01", "web-01"]);
    }
}
