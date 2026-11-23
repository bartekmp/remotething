//! Window-wide keys and pointer events: the app's own shortcuts, and the
//! events behind the context menu, dragging and Escape.

use iced::{Event, Task, keyboard, mouse, window};

use crate::app::{Centre, Message, State};

/// The window's own key combinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shortcut {
    /// Ctrl+Shift+T.
    NewTab,
    /// Ctrl+Shift+L: a shell on this machine in a new tab.
    LocalTerminal,
    /// Ctrl+Shift+W: always closes the tab in front.
    CloseTab,
    /// Ctrl+W: closes the tab in front unless a terminal would want the key.
    CloseTabUnlessTerminal,
    /// Ctrl+F: searches the connections, unless a terminal would want the key.
    SearchConnections,
    /// Ctrl+Shift+P: the command palette.
    Palette,
    /// Ctrl+Shift+S: split the view, or go back to one tab.
    Split,
    /// Ctrl+Shift+R: stack the panes of a split view, or put them side by side.
    Rotate,
    /// Ctrl+Shift+O: the keyboard goes to the other pane of a split view.
    OtherPane,
    /// Ctrl+Plus or Ctrl+=: a bigger terminal text size.
    TextBigger,
    /// Ctrl+Minus: a smaller terminal text size.
    TextSmaller,
    /// F1: the list of shortcuts.
    Help,
    /// Ctrl+Tab.
    NextTab,
    /// Ctrl+Shift+Tab.
    PreviousTab,
    /// Alt+1 to Alt+9: the tab with that number, 9 being the last.
    GoToTab(usize),
}

impl Shortcut {
    pub(crate) fn of(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Self> {
        if *key == keyboard::Key::Named(keyboard::key::Named::F1) && modifiers.is_empty() {
            return Some(Shortcut::Help);
        }
        if *key == keyboard::Key::Named(keyboard::key::Named::Tab)
            && modifiers.control()
            && !modifiers.alt()
        {
            return Some(if modifiers.shift() {
                Shortcut::PreviousTab
            } else {
                Shortcut::NextTab
            });
        }
        let keyboard::Key::Character(character) = key else {
            return None;
        };
        if modifiers.alt() && !modifiers.control() && !modifiers.shift() {
            let number = character.chars().next()?.to_digit(10)?;
            return (1..=9)
                .contains(&number)
                .then_some(Shortcut::GoToTab(number as usize));
        }
        if !modifiers.control() || modifiers.alt() {
            return None;
        }
        match (character.to_lowercase().as_str(), modifiers.shift()) {
            ("t", true) => Some(Shortcut::NewTab),
            ("l", true) => Some(Shortcut::LocalTerminal),
            ("w", true) => Some(Shortcut::CloseTab),
            ("p", true) => Some(Shortcut::Palette),
            ("s", true) => Some(Shortcut::Split),
            ("o", true) => Some(Shortcut::OtherPane),
            ("r", true) => Some(Shortcut::Rotate),
            ("+" | "=", _) => Some(Shortcut::TextBigger),
            ("-" | "_", _) => Some(Shortcut::TextSmaller),
            ("w", false) => Some(Shortcut::CloseTabUnlessTerminal),
            ("f", false) => Some(Shortcut::SearchConnections),
            _ => None,
        }
    }

    /// Whether a remote desktop in front lets the app have the key. Those that a desktop
    /// application could use (Ctrl+W, Ctrl+F) go to the desktop.
    pub(crate) fn beats_a_desktop(self) -> bool {
        !matches!(
            self,
            Shortcut::CloseTabUnlessTerminal
                | Shortcut::SearchConnections
                | Shortcut::TextBigger
                | Shortcut::TextSmaller
        )
    }
}

pub(crate) fn on_shortcut(state: &mut State, shortcut: Shortcut, captured: bool) -> Task<Message> {
    match shortcut {
        Shortcut::NewTab => {
            state.centre = Centre::Sessions;
            state.tabs.open_launcher().map(Message::Tabs)
        }
        Shortcut::LocalTerminal => {
            state.centre = Centre::Sessions;
            state.open_local_terminal()
        }
        Shortcut::CloseTab => {
            state.tabs.close_front();
            Task::none()
        }
        Shortcut::Palette => {
            state.palette = Some(crate::palette::State::default());
            iced::widget::operation::focus(crate::palette::INPUT)
        }
        Shortcut::Split => {
            state.centre = Centre::Sessions;
            state.tabs.toggle_split().map(Message::Tabs)
        }
        Shortcut::Rotate => {
            state.tabs.rotate_split();
            Task::none()
        }
        Shortcut::OtherPane => {
            state.tabs.focus_other_pane();
            Task::none()
        }
        Shortcut::Help => {
            state.shortcuts_open = true;
            Task::none()
        }
        Shortcut::NextTab | Shortcut::PreviousTab => {
            state.centre = Centre::Sessions;
            state.tabs.cycle(shortcut == Shortcut::NextTab);
            Task::none()
        }
        Shortcut::TextBigger | Shortcut::TextSmaller => {
            if state.centre == Centre::Sessions && state.tabs.front_is_terminal() {
                state.zoom_terminal(if shortcut == Shortcut::TextBigger {
                    1
                } else {
                    -1
                });
            }
            Task::none()
        }
        Shortcut::GoToTab(number) => {
            state.centre = Centre::Sessions;
            state.tabs.activate_number(number);
            Task::none()
        }
        Shortcut::SearchConnections => {
            // A shell uses Ctrl+F to move forward, so a live terminal keeps the key.
            if state.centre == Centre::Sessions && state.tabs.front_is_terminal() {
                if captured {
                    return Task::none();
                }
                let key = keyboard::Key::Character("f".into());
                return state
                    .tabs
                    .key(&key, keyboard::Modifiers::CTRL, None)
                    .map(Message::Tabs);
            }
            state.preferences.tree_collapsed = false;
            state.tree.open_search();
            iced::widget::operation::focus(crate::tree::SEARCH_INPUT)
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

/// The keys while the palette is open: arrows move, Escape closes, the shortcut closes it
/// again, and nothing reaches the terminal or desktop behind it.
pub(crate) fn on_palette_key(
    state: &mut State,
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Task<Message> {
    use keyboard::key::Named;
    if Shortcut::of(key, modifiers) == Some(Shortcut::Palette) {
        state.palette = None;
        return Task::none();
    }
    let count = match &state.palette {
        Some(palette) => {
            crate::palette::entries(&state.store, state.recent.ids(), palette.query()).len()
        }
        None => 0,
    };
    let Some(palette) = state.palette.as_mut() else {
        return Task::none();
    };
    match key {
        keyboard::Key::Named(Named::Escape) => state.palette = None,
        keyboard::Key::Named(Named::ArrowDown) => palette.step(true, count),
        keyboard::Key::Named(Named::ArrowUp) => palette.step(false, count),
        _ => {}
    }
    Task::none()
}

/// The keys while the shortcuts list is open: Escape or F1 close it, and nothing reaches
/// the terminal or desktop behind it.
pub(crate) fn on_shortcuts_key(state: &mut State, key: &keyboard::Key) {
    let escape = *key == keyboard::Key::Named(keyboard::key::Named::Escape);
    let help = Shortcut::of(key, keyboard::Modifiers::empty()) == Some(Shortcut::Help);
    if escape || help {
        state.shortcuts_open = false;
    }
}

/// Does what a chosen line of the palette stands for.
pub(crate) fn open_palette_item(state: &mut State, item: crate::palette::Item) -> Task<Message> {
    use crate::palette::Item;
    match item {
        Item::Connection(id) => {
            state.centre = Centre::Sessions;
            state.open(&id)
        }
        Item::NewTab => on_shortcut(state, Shortcut::NewTab, false),
        Item::LocalTerminal => on_shortcut(state, Shortcut::LocalTerminal, false),
        Item::Credentials => {
            state.centre = Centre::Credentials;
            Task::none()
        }
        Item::Settings => {
            state.centre = Centre::Settings;
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
    match state.tabs.file_dropped(path) {
        Some(message) => Task::done(Message::Tabs(message)),
        None => Task::none(),
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

    fn pressed(key: keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Shortcut> {
        Shortcut::of(&key, modifiers)
    }

    fn character(text: &str) -> keyboard::Key {
        keyboard::Key::Character(text.into())
    }

    #[test]
    fn the_palette_and_tab_keys_are_recognised() {
        let tab = keyboard::Key::Named(keyboard::key::Named::Tab);
        let ctrl_shift = keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT;

        assert_eq!(pressed(character("P"), ctrl_shift), Some(Shortcut::Palette));
        assert_eq!(
            pressed(tab.clone(), keyboard::Modifiers::CTRL),
            Some(Shortcut::NextTab)
        );
        assert_eq!(pressed(tab, ctrl_shift), Some(Shortcut::PreviousTab));
        assert_eq!(
            pressed(character("3"), keyboard::Modifiers::ALT),
            Some(Shortcut::GoToTab(3))
        );
    }

    #[test]
    fn the_split_keys_are_recognised_and_beat_a_desktop() {
        let ctrl_shift = keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT;

        assert_eq!(pressed(character("S"), ctrl_shift), Some(Shortcut::Split));
        assert_eq!(
            pressed(character("O"), ctrl_shift),
            Some(Shortcut::OtherPane)
        );
        assert_eq!(pressed(character("R"), ctrl_shift), Some(Shortcut::Rotate));
        let ctrl = keyboard::Modifiers::CTRL;
        assert_eq!(pressed(character("="), ctrl), Some(Shortcut::TextBigger));
        assert_eq!(
            pressed(character("+"), ctrl_shift),
            Some(Shortcut::TextBigger)
        );
        assert_eq!(pressed(character("-"), ctrl), Some(Shortcut::TextSmaller));
        assert!(!Shortcut::TextBigger.beats_a_desktop());
        assert!(Shortcut::Rotate.beats_a_desktop());
        assert_eq!(pressed(character("s"), keyboard::Modifiers::CTRL), None);
        assert!(Shortcut::Split.beats_a_desktop() && Shortcut::OtherPane.beats_a_desktop());
    }

    #[test]
    fn f1_alone_opens_the_help_even_over_a_desktop() {
        let f1 = keyboard::Key::Named(keyboard::key::Named::F1);

        assert_eq!(
            pressed(f1.clone(), keyboard::Modifiers::empty()),
            Some(Shortcut::Help)
        );
        assert_eq!(pressed(f1, keyboard::Modifiers::CTRL), None);
        assert!(Shortcut::Help.beats_a_desktop());
    }

    #[test]
    fn keys_a_terminal_or_shell_uses_are_left_alone() {
        // Plain Ctrl+P is history in a shell; Alt+0 and Alt letters belong to programs too.
        assert_eq!(pressed(character("p"), keyboard::Modifiers::CTRL), None);
        assert_eq!(pressed(character("0"), keyboard::Modifiers::ALT), None);
        assert_eq!(pressed(character("b"), keyboard::Modifiers::ALT), None);
        assert_eq!(pressed(character("3"), keyboard::Modifiers::empty()), None);
    }

    #[test]
    fn a_remote_desktop_keeps_ctrl_w_and_ctrl_f_but_not_the_tab_keys() {
        assert!(!Shortcut::CloseTabUnlessTerminal.beats_a_desktop());
        assert!(!Shortcut::SearchConnections.beats_a_desktop());
        assert!(Shortcut::NextTab.beats_a_desktop());
        assert!(Shortcut::Palette.beats_a_desktop());
        assert!(Shortcut::GoToTab(2).beats_a_desktop());
    }

    #[test]
    fn a_file_dropped_on_the_window_becomes_a_message_with_its_path() {
        let path = std::path::PathBuf::from("/tmp/report.pdf");
        let event = Event::Window(window::Event::FileDropped(path.clone()));

        let message = window_event(event, iced::event::Status::Ignored, window::Id::unique());

        assert!(matches!(message, Some(Message::FileDropped(dropped)) if dropped == path));
    }
}
