//! Spike for risks R1 and R4: which file-drop and key events reach an iced window.
//!
//! Run it on the desktop under test, then:
//!   1. drag a file from the file manager onto the window (R1)
//!   2. press Alt+Tab, Super and Ctrl+Alt+Del with the window focused (R4)
//!
//! Compare native Wayland with XWayland: `WAYLAND_DISPLAY= spike-wayland-probe`.

use iced::widget::{column, scrollable, text};
use iced::{Element, Event, Font, Subscription, event, keyboard, window};

const MAX_LINES: usize = 200;

#[derive(Default)]
struct App {
    lines: Vec<String>,
}

fn update(app: &mut App, line: String) {
    app.lines.push(line);
    if app.lines.len() > MAX_LINES {
        app.lines.remove(0);
    }
}

fn view(app: &App) -> Element<'_, String> {
    let backend = match std::env::var("WAYLAND_DISPLAY") {
        Ok(display) if !display.is_empty() => "native Wayland",
        _ => "X11 or XWayland",
    };
    let header = text(format!(
        "session: {backend}. Drop a file here, then try Alt+Tab and Super."
    ));
    let log = text(app.lines.join("\n")).font(Font::MONOSPACE).size(13);

    column![header, scrollable(log).anchor_bottom()]
        .spacing(8)
        .padding(8)
        .into()
}

fn subscription(_app: &App) -> Subscription<String> {
    event::listen_with(|event, _status, _window| describe(&event))
}

/// One log line for each event the two risks care about; `None` for the rest.
fn describe(event: &Event) -> Option<String> {
    match event {
        Event::Window(window::Event::FileHovered(path)) => {
            Some(format!("file hovered: {}", path.display()))
        }
        Event::Window(window::Event::FileDropped(path)) => {
            Some(format!("FILE DROPPED: {}", path.display()))
        }
        Event::Window(window::Event::FilesHoveredLeft) => Some("file hover left".to_string()),
        Event::Window(window::Event::Unfocused) => Some("window lost focus".to_string()),
        Event::Window(window::Event::Focused) => Some("window got focus".to_string()),
        Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            Some(format!("key pressed: {key:?} with {modifiers:?}"))
        }
        _ => None,
    }
}

fn main() -> iced::Result {
    iced::application(App::default, update, view)
        .title("spike: file drop and shortcut probe")
        .subscription(subscription)
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_dropped_file_is_logged_with_its_path() {
        let event = Event::Window(window::Event::FileDropped(PathBuf::from("/tmp/a.txt")));
        assert_eq!(
            describe(&event),
            Some("FILE DROPPED: /tmp/a.txt".to_string())
        );
    }

    #[test]
    fn unrelated_events_are_not_logged() {
        let event = Event::Window(window::Event::CloseRequested);
        assert_eq!(describe(&event), None);
    }
}
