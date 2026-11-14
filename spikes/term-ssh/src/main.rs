//! Spike for risk R3: an iced window showing a terminal fed by a russh channel.
//!
//! Usage: spike-term-ssh [addr] [user] [password] [--insecure]
//! Defaults match `spikes/servers/compose.yml`.

use iced::futures::{SinkExt, Stream};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{container, text};
use iced::{Element, Event, Font, Subscription, event, stream};
use spike_term_ssh::screen::Screen;
use spike_term_ssh::ssh::open_shell;
use tokio::sync::mpsc;

const COLUMNS: usize = 100;
const LINES: usize = 30;

#[derive(Debug, Clone)]
enum Message {
    Connected(mpsc::Sender<Vec<u8>>),
    Output(Vec<u8>),
    Typed(Vec<u8>),
    Failed(String),
}

struct App {
    screen: Screen,
    input: Option<mpsc::Sender<Vec<u8>>>,
    error: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::new(COLUMNS, LINES),
            input: None,
            error: None,
        }
    }
}

fn update(app: &mut App, message: Message) {
    match message {
        Message::Connected(input) => app.input = Some(input),
        Message::Output(bytes) => app.screen.feed(&bytes),
        Message::Typed(bytes) => {
            if let Some(input) = &app.input {
                // Dropping keystrokes when the channel is full is fine for a spike.
                let _ = input.try_send(bytes);
            }
        }
        Message::Failed(reason) => app.error = Some(reason),
    }
}

fn view(app: &App) -> Element<'_, Message> {
    let content = match &app.error {
        Some(reason) => format!("error: {reason}"),
        None => app.screen.text(),
    };
    container(text(content).font(Font::MONOSPACE).size(14))
        .padding(8)
        .into()
}

fn subscription(_app: &App) -> Subscription<Message> {
    Subscription::batch([
        Subscription::run(shell_stream),
        event::listen_with(|event, _status, _window| match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) => key_to_bytes(&key, modifiers, text.as_deref()).map(Message::Typed),
            _ => None,
        }),
    ])
}

/// Connects once and forwards shell output into the UI as messages.
fn shell_stream() -> impl Stream<Item = Message> {
    stream::channel(64, async |mut ui| {
        let all: Vec<String> = std::env::args().skip(1).collect();
        let insecure = all.iter().any(|argument| argument == "--insecure");
        let args: Vec<&String> = all.iter().filter(|a| !a.starts_with("--")).collect();
        let argument = |index: usize, default: &str| {
            args.get(index)
                .map(|value| value.to_string())
                .unwrap_or_else(|| default.to_string())
        };
        let addr = argument(0, "127.0.0.1:2222");
        let user = argument(1, "spike");
        let password = argument(2, "spike");

        let shell = open_shell(
            &addr,
            &user,
            &password,
            COLUMNS as u32,
            LINES as u32,
            insecure,
        )
        .await;
        let mut shell = match shell {
            Ok(shell) => shell,
            Err(error) => {
                let _ = ui.send(Message::Failed(error.to_string())).await;
                return;
            }
        };

        let _ = ui.send(Message::Connected(shell.input.clone())).await;
        while let Some(bytes) = shell.output.recv().await {
            if ui.send(Message::Output(bytes)).await.is_err() {
                return;
            }
        }
        let _ = ui.send(Message::Failed("connection closed".into())).await;
    })
}

/// The smallest key mapping that makes a shell usable.
fn key_to_bytes(key: &Key, modifiers: keyboard::Modifiers, text: Option<&str>) -> Option<Vec<u8>> {
    if let Key::Named(named) = key {
        let bytes: &[u8] = match named {
            Named::Enter => b"\r",
            Named::Backspace => b"\x7f",
            Named::Tab => b"\t",
            Named::Escape => b"\x1b",
            Named::ArrowUp => b"\x1b[A",
            Named::ArrowDown => b"\x1b[B",
            Named::ArrowRight => b"\x1b[C",
            Named::ArrowLeft => b"\x1b[D",
            Named::Space => b" ",
            _ => return None,
        };
        return Some(bytes.to_vec());
    }

    if modifiers.control() {
        // Ctrl+A..Z map to control codes 1..26.
        if let Key::Character(character) = key {
            let letter = character.chars().next()?.to_ascii_lowercase();
            if letter.is_ascii_lowercase() {
                return Some(vec![letter as u8 - b'a' + 1]);
            }
        }
        return None;
    }

    text.map(|typed| typed.as_bytes().to_vec())
}

fn main() -> iced::Result {
    iced::application(App::default, update, view)
        .title("spike: terminal over russh")
        .subscription(subscription)
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_becomes_the_interrupt_byte() {
        let key = Key::Character("c".into());
        let bytes = key_to_bytes(&key, keyboard::Modifiers::CTRL, Some("c"));
        assert_eq!(bytes, Some(vec![3]));
    }

    #[test]
    fn enter_becomes_carriage_return() {
        let bytes = key_to_bytes(
            &Key::Named(Named::Enter),
            keyboard::Modifiers::empty(),
            None,
        );
        assert_eq!(bytes, Some(b"\r".to_vec()));
    }

    #[test]
    fn printable_text_passes_through() {
        let key = Key::Character("ą".into());
        let bytes = key_to_bytes(&key, keyboard::Modifiers::empty(), Some("ą"));
        assert_eq!(bytes, Some("ą".as_bytes().to_vec()));
    }
}
