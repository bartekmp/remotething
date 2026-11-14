//! The tabbed sessions in the centre. No protocol is wired yet, so a tab
//! shows what the connection resolves to.

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};
use remotething_core::ResolvedConnection;

use crate::i18n::fl;
use crate::icons;
use crate::theme;
use crate::tree::protocol_label;

#[derive(Debug, Default)]
pub struct State {
    tabs: Vec<ResolvedConnection>,
    active: usize,
}

#[derive(Debug, Clone)]
pub enum Message {
    Activated(usize),
    Closed(usize),
}

impl State {
    /// Opens a new tab for the connection and brings it to the front.
    pub fn open(&mut self, connection: ResolvedConnection) {
        self.tabs.push(connection);
        self.active = self.tabs.len() - 1;
    }

    #[cfg(test)]
    pub fn open_ids(&self) -> Vec<&str> {
        self.tabs.iter().map(|tab| tab.id.as_str()).collect()
    }
}

pub fn update(state: &mut State, message: Message) {
    match message {
        Message::Activated(index) => {
            if index < state.tabs.len() {
                state.active = index;
            }
        }
        Message::Closed(index) => {
            if index >= state.tabs.len() {
                return;
            }
            state.tabs.remove(index);
            // Keep the same tab in front; when it was the one closed, its left neighbour.
            if state.active > index || state.active >= state.tabs.len() {
                state.active = state.active.saturating_sub(1);
            }
        }
    }
}

pub fn view(state: &State) -> Element<'_, Message> {
    let Some(active) = state.tabs.get(state.active) else {
        return container(text(fl!("tabs-empty")).style(theme::secondary_text))
            .center(Length::Fill)
            .into();
    };

    let strip = row(state.tabs.iter().enumerate().map(|(index, tab)| {
        let is_active = index == state.active;
        row![
            button(text(tab.name.as_str()))
                .style(theme::entry(is_active))
                .on_press(Message::Activated(index)),
            button(icons::view(icons::CLOSE, theme::TEXT_BODY))
                .style(theme::entry(false))
                .on_press(Message::Closed(index)),
        ]
        .into()
    }))
    .spacing(theme::GAP);

    column![strip, view_session(active)]
        .spacing(12)
        .width(Length::Fill)
        .into()
}

fn view_session(connection: &ResolvedConnection) -> Element<'_, Message> {
    let address = fl!(
        "session-address",
        protocol = protocol_label(connection.protocol),
        host = connection.host.as_str(),
        port = connection.port.to_string()
    );
    let credential = match &connection.credential {
        Some(id) => fl!("session-credential", credential = id.as_str()),
        None => fl!("session-no-credential"),
    };

    let mut lines = column![
        text(connection.name.as_str()).size(theme::TEXT_DISPLAY),
        text(address),
        text(credential),
    ]
    .spacing(8);
    if let Some(jump) = &connection.jump {
        lines = lines.push(text(fl!("session-jump", jump = jump.as_str())));
    }
    lines
        .push(text(fl!("session-not-implemented")).style(theme::secondary_text))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use remotething_core::{Protocol, Quality};

    fn connection(id: &str) -> ResolvedConnection {
        ResolvedConnection {
            id: id.to_string(),
            name: id.to_string(),
            protocol: Protocol::Ssh,
            host: format!("{id}.example.net"),
            port: 22,
            credential: None,
            jump: None,
            rdp_quality: Quality::Balanced,
        }
    }

    fn state_with(ids: &[&str]) -> State {
        let mut state = State::default();
        for id in ids {
            state.open(connection(id));
        }
        state
    }

    fn active_id(state: &State) -> Option<&str> {
        state.tabs.get(state.active).map(|tab| tab.id.as_str())
    }

    #[test]
    fn a_new_tab_comes_to_the_front() {
        let state = state_with(&["a", "b"]);

        assert_eq!(active_id(&state), Some("b"));
    }

    #[test]
    fn activating_switches_tabs() {
        let mut state = state_with(&["a", "b"]);

        update(&mut state, Message::Activated(0));

        assert_eq!(active_id(&state), Some("a"));
    }

    #[test]
    fn closing_a_tab_to_the_left_keeps_the_active_one() {
        let mut state = state_with(&["a", "b", "c"]);

        update(&mut state, Message::Closed(0));

        assert_eq!(active_id(&state), Some("c"));
    }

    #[test]
    fn closing_the_active_last_tab_moves_left() {
        let mut state = state_with(&["a", "b", "c"]);

        update(&mut state, Message::Closed(2));

        assert_eq!(active_id(&state), Some("b"));
    }

    #[test]
    fn closing_the_only_tab_leaves_none() {
        let mut state = state_with(&["a"]);

        update(&mut state, Message::Closed(0));

        assert_eq!(active_id(&state), None);
    }

    #[test]
    fn an_out_of_range_index_is_ignored() {
        let mut state = state_with(&["a"]);

        update(&mut state, Message::Closed(7));
        update(&mut state, Message::Activated(7));

        assert_eq!(active_id(&state), Some("a"));
    }
}
