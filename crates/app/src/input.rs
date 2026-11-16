//! Window-wide keys and pointer events: the app's own shortcuts, and the
//! events behind the context menu, dragging and Escape.

use iced::{Event, Task, keyboard, mouse, window};

use crate::app::{Centre, Message, State};
use crate::files_pane;
use crate::session;
use crate::tabs;

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

/// A file dropped on the window is uploaded when a session with its file pane
/// open is in front. It goes the same way as a picked file, so a name that is
/// already there asks before replacing it.
pub(crate) fn on_file_dropped(state: &State, path: std::path::PathBuf) -> Task<Message> {
    if state.centre != Centre::Sessions {
        return Task::none();
    }
    let Some(tab) = state.tabs.front_with_files() else {
        return Task::none();
    };
    let upload = files_pane::Message::UploadPicked(Some(path));
    let message = tabs::Message::Session(tab, session::Message::Files(upload));
    Task::done(Message::Tabs(message))
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
        Event::Window(window::Event::FileDropped(path)) => Some(Message::FileDropped(path)),
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
        Event::Keyboard(keyboard::Event::KeyReleased {
            key, physical_key, ..
        }) => Some(Message::KeyReleased {
            key,
            physical: physical_key,
        }),
        Event::Window(window::Event::Rescaled(scale)) => Some(Message::ScaleChanged(scale)),
        Event::Window(window::Event::Opened { size, .. })
        | Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_dropped_on_the_window_becomes_a_message_with_its_path() {
        let path = std::path::PathBuf::from("/tmp/report.pdf");
        let event = Event::Window(window::Event::FileDropped(path.clone()));

        let message = window_event(event, iced::event::Status::Ignored, window::Id::unique());

        assert!(matches!(message, Some(Message::FileDropped(dropped)) if dropped == path));
    }
}
