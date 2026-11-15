//! The screen that previews a Remotething import: the connections in the
//! file, each with a tick. Nothing is written until the user presses Import.

use std::collections::HashSet;

use iced::widget::{button, checkbox, column, row, scrollable, text};
use iced::{Element, Length};
use remotething_core::Store;

use crate::i18n::fl;
use crate::theme;

pub struct State {
    /// What the file holds.
    incoming: Store,
    /// The connections ticked for import, by id.
    chosen: HashSet<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Toggled(String, bool),
    AllPressed,
    NonePressed,
    ImportPressed,
    BackPressed,
}

/// What the screen asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Add this tree to the user's tree.
    Apply(Store),
    Close,
}

impl State {
    /// A screen for the file's tree, with every connection ticked.
    pub fn new(incoming: Store) -> Self {
        let chosen = incoming.connections.iter().map(|c| c.id.clone()).collect();
        Self { incoming, chosen }
    }

    /// The tree to add: the whole file when everything is ticked, so empty
    /// folders and unused credentials come along; else just the ticked ones.
    fn selection(&self) -> Store {
        if self.chosen.len() == self.incoming.connections.len() {
            return self.incoming.clone();
        }
        let ids: Vec<String> = self.chosen.iter().cloned().collect();
        self.incoming.pick_connections(&ids)
    }

    /// The folders a connection sits in, outermost first, like `Prod / EU`.
    fn folder_path(&self, parent: Option<&str>) -> String {
        let mut names = Vec::new();
        let mut next = parent;
        // The file was validated, so the chain ends; the cap guards a hand-built one.
        for _ in 0..=self.incoming.folders.len() {
            let Some(id) = next else { break };
            let Some(folder) = self.incoming.folders.iter().find(|f| f.id == id) else {
                break;
            };
            names.push(folder.name.as_str());
            next = folder.parent.as_deref();
        }
        names.reverse();
        names.join(" / ")
    }
}

pub fn update(state: &mut State, message: Message) -> Option<Event> {
    match message {
        Message::Toggled(id, ticked) => {
            if ticked {
                state.chosen.insert(id);
            } else {
                state.chosen.remove(&id);
            }
        }
        Message::AllPressed => {
            state.chosen = state
                .incoming
                .connections
                .iter()
                .map(|c| c.id.clone())
                .collect();
        }
        Message::NonePressed => state.chosen.clear(),
        Message::ImportPressed if !state.chosen.is_empty() => {
            return Some(Event::Apply(state.selection()));
        }
        Message::ImportPressed => {}
        Message::BackPressed => return Some(Event::Close),
    }
    None
}

pub fn view(state: &State) -> Element<'_, Message> {
    let header = row![
        text(fl!("preview-title"))
            .size(theme::TEXT_DISPLAY)
            .width(Length::Fill),
        button(text(fl!("putty-back")))
            .style(theme::pill)
            .on_press(Message::BackPressed),
    ];
    let mut list = column![].spacing(8);
    if state.incoming.connections.is_empty() {
        list = list.push(text(fl!("preview-none")).style(theme::secondary_text));
    }
    for connection in &state.incoming.connections {
        let id = connection.id.clone();
        let place = state.folder_path(connection.parent.as_deref());
        let label = if place.is_empty() {
            format!("{}  ({})", connection.name, connection.host)
        } else {
            format!("{}  ({})  {place}", connection.name, connection.host)
        };
        list = list.push(
            checkbox(state.chosen.contains(&connection.id))
                .label(label)
                .on_toggle(move |on| Message::Toggled(id.clone(), on)),
        );
    }
    let import = button(text(fl!("preview-import", count = state.chosen.len())))
        .style(theme::primary)
        .on_press_maybe((!state.chosen.is_empty()).then_some(Message::ImportPressed));
    let buttons = row![
        import,
        button(text(fl!("preview-all")))
            .style(theme::pill)
            .on_press(Message::AllPressed),
        button(text(fl!("preview-none-button")))
            .style(theme::pill)
            .on_press(Message::NonePressed),
    ]
    .spacing(8);
    let body = column![
        text(fl!("preview-explain")).style(theme::secondary_text),
        list,
        buttons,
    ]
    .spacing(10);
    column![header, scrollable(body).height(Length::Fill)]
        .spacing(16)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn incoming() -> Store {
        toml::from_str(
            r#"
            [[folder]]
            id = "prod"
            name = "Prod"

            [[folder]]
            id = "empty"
            name = "Empty"

            [[connection]]
            id = "web"
            name = "web"
            protocol = "ssh"
            host = "web"
            parent = "prod"

            [[connection]]
            id = "db"
            name = "db"
            protocol = "ssh"
            host = "db"
            "#,
        )
        .unwrap()
    }

    #[test]
    fn importing_everything_keeps_the_whole_file_even_empty_folders() {
        let mut state = State::new(incoming());

        let event = update(&mut state, Message::ImportPressed);

        match event {
            Some(Event::Apply(tree)) => assert_eq!(tree.folders.len(), 2),
            other => panic!("expected an import, got {other:?}"),
        }
    }

    #[test]
    fn unticking_leaves_a_connection_and_its_unneeded_folder_out() {
        let mut state = State::new(incoming());

        let _ = update(&mut state, Message::Toggled("web".to_string(), false));
        let event = update(&mut state, Message::ImportPressed);

        match event {
            Some(Event::Apply(tree)) => {
                assert_eq!(tree.connections.len(), 1);
                assert!(tree.folders.is_empty());
            }
            other => panic!("expected an import, got {other:?}"),
        }
    }

    #[test]
    fn nothing_ticked_imports_nothing() {
        let mut state = State::new(incoming());

        let _ = update(&mut state, Message::NonePressed);

        assert_eq!(update(&mut state, Message::ImportPressed), None);
    }

    #[test]
    fn a_connection_shows_the_folders_it_sits_in() {
        let state = State::new(incoming());

        assert_eq!(state.folder_path(Some("prod")), "Prod");
        assert_eq!(state.folder_path(None), "");
    }
}
