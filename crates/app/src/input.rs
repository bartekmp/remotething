//! Window-wide keys and pointer events: the app's own shortcuts, and the
//! events behind the context menu, dragging and Escape.

use iced::{Event, Task, keyboard, mouse, window};

use crate::app::{Centre, Message, State};

/// The window's own key combinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shortcut {
    /// Ctrl+Shift+T.
    NewTab,
    /// Ctrl+Shift+W: always closes the tab in front.
    CloseTab,
    /// Ctrl+W: closes the tab in front unless a terminal would want the key.
    CloseTabUnlessTerminal,
}

impl Shortcut {
    pub(crate) fn of(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Self> {
        let keyboard::Key::Character(character) = key else {
            return None;
        };
        if !modifiers.control() || modifiers.alt() {
            return None;
        }
        match (character.to_lowercase().as_str(), modifiers.shift()) {
            ("t", true) => Some(Shortcut::NewTab),
            ("w", true) => Some(Shortcut::CloseTab),
            ("w", false) => Some(Shortcut::CloseTabUnlessTerminal),
            _ => None,
        }
    }
}

pub(crate) fn on_shortcut(state: &mut State, shortcut: Shortcut, captured: bool) -> Task<Message> {
    match shortcut {
        Shortcut::NewTab => {
            state.centre = Centre::Sessions;
            state.tabs.open_launcher().map(Message::Tabs)
        }
        Shortcut::CloseTab => {
            state.tabs.close_front();
            Task::none()
        }
        Shortcut::CloseTabUnlessTerminal => {
            // Shells use Ctrl+W to delete a word, so a live terminal keeps the key.
            if state.centre == Centre::Sessions && state.tabs.front_is_terminal() {
                if captured {
                    return Task::none();
                }
                let key = keyboard::Key::Character("w".into());
                return state
                    .tabs
                    .key(&key, keyboard::Modifiers::CTRL, None)
                    .map(Message::Tabs);
            }
            state.tabs.close_front();
            Task::none()
        }
    }
}

/// The window-wide events behind the context menu, dragging and Escape.
pub(crate) fn window_event(
    event: Event,
    status: iced::event::Status,
    _window: window::Id,
) -> Option<Message> {
    match event {
        Event::Mouse(mouse::Event::CursorMoved { position }) => {
            Some(Message::PointerMoved(position))
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(Message::PointerReleased)
        }
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            text,
            ..
        }) => Some(Message::KeyPressed {
            key,
            physical: physical_key,
            modifiers,
            text: text.map(|text| text.to_string()),
            captured: status == iced::event::Status::Captured,
        }),
        Event::Keyboard(keyboard::Event::KeyReleased { physical_key, .. }) => {
            Some(Message::KeyReleased {
                physical: physical_key,
            })
        }
        Event::Window(window::Event::Opened { size, .. })
        | Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
        _ => None,
    }
}
