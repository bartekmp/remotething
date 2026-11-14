//! Spike for risk R2: an IronRDP desktop drawn in an iced window.
//!
//! Usage: spike-rdp-frame [host] [port] [user] [password] [--nla] [--insecure]
//! Defaults match `spikes/servers/compose.yml`.
//! `SPIKE_RDP_PASSWORD` overrides the password, to keep it out of shell history.

use iced::futures::{SinkExt, Stream};
use iced::widget::image::Handle;
use iced::widget::{center, image, text};
use iced::{ContentFit, Element, Length, Subscription, stream};
use spike_rdp_frame::session::{Target, run};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
enum Message {
    Frame(Handle),
    Ended(String),
}

#[derive(Default)]
struct App {
    frame: Option<Handle>,
    status: Option<String>,
}

fn update(app: &mut App, message: Message) {
    match message {
        Message::Frame(frame) => app.frame = Some(frame),
        Message::Ended(reason) => app.status = Some(reason),
    }
}

fn view(app: &App) -> Element<'_, Message> {
    if let Some(reason) = &app.status {
        return center(text(reason.as_str())).into();
    }
    match &app.frame {
        Some(frame) => image(frame.clone())
            .content_fit(ContentFit::Contain)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        None => center(text("connecting...")).into(),
    }
}

fn subscription(_app: &App) -> Subscription<Message> {
    Subscription::run(session_stream)
}

fn target_from_args() -> Target {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let nla = all.iter().any(|argument| argument == "--nla");
    let insecure = all.iter().any(|argument| argument == "--insecure");
    let positional: Vec<&String> = all.iter().filter(|a| !a.starts_with("--")).collect();
    let argument = |index: usize, default: &str| {
        positional
            .get(index)
            .map(|value| value.to_string())
            .unwrap_or_else(|| default.to_string())
    };

    Target {
        host: argument(0, "127.0.0.1"),
        port: argument(1, "3389").parse().unwrap_or(3389),
        username: argument(2, "abc"),
        password: std::env::var("SPIKE_RDP_PASSWORD").unwrap_or_else(|_| argument(3, "abc")),
        nla,
        insecure,
    }
}

/// Runs the blocking session on its own thread and forwards frames to the UI.
fn session_stream() -> impl Stream<Item = Message> {
    stream::channel(2, async |mut ui| {
        let (frames, mut received) = mpsc::channel(2);

        std::thread::spawn(move || {
            let target = target_from_args();
            let ending = frames.clone();
            let result = run(target, |frame| {
                let handle = Handle::from_rgba(frame.width, frame.height, frame.rgba);
                frames.blocking_send(Message::Frame(handle)).is_ok()
            });
            let reason = match result {
                Ok(()) => "disconnected".to_string(),
                Err(error) => format!("error: {error:#}"),
            };
            let _ = ending.blocking_send(Message::Ended(reason));
        });

        while let Some(message) = received.recv().await {
            if ui.send(message).await.is_err() {
                return;
            }
        }
    })
}

fn main() -> iced::Result {
    iced::application(App::default, update, view)
        .title("spike: IronRDP frame")
        .subscription(subscription)
        .run()
}
