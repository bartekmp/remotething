//! Connecting, and the task that runs a session.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use remotething_secrets::Secret;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use vnc::{
    ClientKeyEvent, ClientMouseEvent, VncClient, VncConnector, VncEncoding, VncError, VncEvent,
    X11Event,
};

use crate::error::{Error, Result};
use crate::framebuffer::{Area, Framebuffer};
use crate::input::{Input, Pointer, latin1};
use crate::quality::{Depth, Quality};
use crate::traffic::{Counted, Counters};

/// The most pictures sent to the window per second.
const FRAME_INTERVAL: Duration = Duration::from_millis(33);
/// How often the server is asked for what changed.
const REFRESH_EVERY: Duration = Duration::from_millis(33);
/// How often the totals are reported.
const STATS_EVERY: Duration = Duration::from_secs(1);
/// How long to rest when nothing happened.
const IDLE: Duration = Duration::from_millis(4);
/// How many pictures or inputs may wait before the sender has to catch up.
const QUEUE: usize = 4;
const INPUT_QUEUE: usize = 256;

/// Where to connect.
pub struct Target {
    /// The host name or address.
    pub host: String,
    /// The TCP port, usually 5900.
    pub port: u16,
    /// The password, if the server wants one.
    pub password: Option<Secret>,
    /// How to trade picture quality for bandwidth.
    pub quality: Quality,
}

/// Facts about a connection that do not change while it lasts.
#[derive(Debug, Clone)]
pub struct Info {
    /// The server's address. `None` when the stream was not a plain TCP connection.
    pub peer: Option<SocketAddr>,
    /// How the connection is secured.
    pub security: &'static str,
    /// The picture encodings offered, best first.
    pub encodings: &'static str,
    /// How many bits a pixel takes on the wire.
    pub colours: &'static str,
}

/// A full copy of the remote desktop, 4 bytes per pixel, opaque.
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Red, green, blue and alpha of every pixel, row by row.
    pub rgba: Vec<u8>,
}

/// The mouse pointer the server wants drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// The pixel that sits on the pointer position, from the left.
    pub hotspot_x: u16,
    /// The pixel that sits on the pointer position, from the top.
    pub hotspot_y: u16,
    /// Red, green, blue and alpha of every pixel, row by row.
    pub rgba: Vec<u8>,
}

/// Running totals since the session began.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Bytes received from the server.
    pub received: u64,
    /// Bytes sent to the server.
    pub sent: u64,
    /// Pictures passed to the window.
    pub frames: u64,
}

/// What a session tells the window.
pub enum Event {
    /// The screen changed. Only the newest frame matters.
    Frame(Frame),
    /// The pointer changed; `None` means no pointer at all.
    Cursor(Option<Cursor>),
    /// Text was copied on the server.
    Clipboard(String),
    /// Running totals, about once per second.
    Stats(Stats),
    /// The session ended, with the reason when it was not the user's doing.
    Closed(Option<String>),
}

/// A running session. It ends when this is dropped.
pub struct Session {
    /// What is known about the connection.
    pub info: Info,
    /// Pictures and the end of the session, in order.
    pub events: mpsc::Receiver<Event>,
    /// Where the user's input goes.
    pub input: mpsc::Sender<Input>,
}

/// Connects over TCP, signs in and starts the session.
pub async fn connect(target: Target) -> Result<Session> {
    let stream = TcpStream::connect((target.host.as_str(), target.port))
        .await
        .map_err(|error| Error::Unreachable {
            host: target.host.clone(),
            port: target.port,
            reason: error.to_string(),
        })?;
    let peer = stream.peer_addr().ok();
    connect_over(stream, target.password, target.quality, peer).await
}

/// Signs in over `stream`, which can be any connection to a VNC server, and starts the session.
pub async fn connect_over<S>(
    stream: S,
    password: Option<Secret>,
    quality: Quality,
    peer: Option<SocketAddr>,
) -> Result<Session>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let counters = Counters::default();
    let password = password.map(|secret| secret.expose().to_string());
    let depth = quality.depth();
    let mut connector =
        VncConnector::new(Counted::new(crate::tight::relay(stream), counters.clone()))
            .set_auth_method(async move { password.ok_or(VncError::NoPassword) })
            .add_encoding(VncEncoding::Zrle)
            .add_encoding(VncEncoding::CopyRect)
            .add_encoding(VncEncoding::Raw);
    if quality.shows_pointer() {
        connector = connector.add_encoding(VncEncoding::CursorPseudo);
    }
    let client = connector
        .add_encoding(VncEncoding::DesktopSizePseudo)
        .allow_shared(true)
        .set_pixel_format(depth.pixel_format())
        .build()?
        .try_start()
        .await?
        .finish()?;

    let (events_tx, events) = mpsc::channel(QUEUE);
    let (input, input_rx) = mpsc::channel(INPUT_QUEUE);
    tokio::spawn(async move {
        let reason = Worker::new(client, counters, depth)
            .run(&events_tx, input_rx)
            .await;
        let _ = events_tx.send(Event::Closed(reason)).await;
    });
    Ok(Session {
        info: Info {
            peer,
            security: "VNC password, not encrypted",
            encodings: "ZRLE, CopyRect, Raw",
            colours: depth.label(),
        },
        events,
        input,
    })
}

struct Worker {
    client: VncClient,
    counters: Counters,
    picture: Framebuffer,
    /// How pixels arrive from the server.
    depth: Depth,
    pointer: Pointer,
    /// The picture changed and has not been sent to the window.
    dirty: bool,
    frames: u64,
    /// Changes the window has not been told yet.
    cursor: Option<Option<Cursor>>,
    copied: Vec<String>,
}

impl Worker {
    fn new(client: VncClient, counters: Counters, depth: Depth) -> Self {
        Self {
            client,
            counters,
            picture: Framebuffer::new(1, 1),
            depth,
            pointer: Pointer::default(),
            dirty: false,
            frames: 0,
            cursor: None,
            copied: Vec::new(),
        }
    }

    /// Runs until the server ends the session or the window drops it.
    /// Returns the reason when it was the server's doing.
    async fn run(
        mut self,
        events: &mpsc::Sender<Event>,
        mut input: mpsc::Receiver<Input>,
    ) -> Option<String> {
        let mut last_frame = Instant::now();
        let mut last_refresh = Instant::now();
        let mut last_stats = Instant::now();
        loop {
            let mut busy = false;
            loop {
                match self.client.poll_event().await {
                    Ok(Some(event)) => {
                        busy = true;
                        if let Err(reason) = self.apply(event) {
                            return Some(reason);
                        }
                    }
                    Ok(None) => break,
                    Err(error) => return Some(closed_reason(&error)),
                }
            }
            loop {
                match input.try_recv() {
                    Ok(one) => {
                        busy = true;
                        if let Err(error) = self.send(one).await {
                            return Some(closed_reason(&error));
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    // The window dropped the session.
                    Err(mpsc::error::TryRecvError::Disconnected) => return None,
                }
            }
            if last_refresh.elapsed() >= REFRESH_EVERY {
                last_refresh = Instant::now();
                if let Err(error) = self.client.input(X11Event::Refresh).await {
                    return Some(closed_reason(&error));
                }
            }
            if self.dirty && last_frame.elapsed() >= FRAME_INTERVAL && self.send_frame(events) {
                last_frame = Instant::now();
            }
            self.send_changes(events);
            if last_stats.elapsed() >= STATS_EVERY {
                last_stats = Instant::now();
                let _ = events.try_send(Event::Stats(self.stats()));
            }
            if events.is_closed() {
                return None;
            }
            if !busy {
                tokio::time::sleep(IDLE).await;
            }
        }
    }

    /// Takes in what the server sent. An error carries the reason to end the session.
    fn apply(&mut self, event: VncEvent) -> std::result::Result<(), String> {
        match event {
            VncEvent::SetResolution(screen) => {
                self.picture = Framebuffer::new(screen.width, screen.height);
                self.dirty = true;
            }
            VncEvent::RawImage(rect, data) => {
                self.picture.paint_depth(area(rect), &data, self.depth);
                self.dirty = true;
            }
            VncEvent::Copy(target, source) => {
                self.picture.copy(area(target), area(source));
                self.dirty = true;
            }
            VncEvent::SetCursor(rect, data) => {
                let cursor = (!data.is_empty()).then_some(Cursor {
                    width: rect.width,
                    height: rect.height,
                    // For a cursor the position of the rectangle is the hotspot.
                    hotspot_x: rect.x,
                    hotspot_y: rect.y,
                    rgba: data,
                });
                self.cursor = Some(cursor);
            }
            VncEvent::Text(text) => self.copied.push(text),
            VncEvent::Error(message) => return Err(message),
            _ => {}
        }
        Ok(())
    }

    async fn send(&mut self, input: Input) -> std::result::Result<(), VncError> {
        for (x, y, buttons) in self.pointer.events(&input) {
            let event = ClientMouseEvent {
                position_x: x,
                position_y: y,
                bottons: buttons,
            };
            self.client.input(X11Event::PointerEvent(event)).await?;
        }
        match input {
            Input::Key { keysym, pressed } => {
                let event = ClientKeyEvent {
                    keycode: keysym,
                    down: pressed,
                };
                self.client.input(X11Event::KeyEvent(event)).await?;
            }
            Input::Clipboard(text) => {
                self.client.input(X11Event::CopyText(latin1(&text))).await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Passes the picture on. False when the window is behind and the picture was skipped.
    fn send_frame(&mut self, events: &mpsc::Sender<Event>) -> bool {
        let frame = Frame {
            width: u32::from(self.picture.width()),
            height: u32::from(self.picture.height()),
            rgba: self.picture.pixels().to_vec(),
        };
        match events.try_send(Event::Frame(frame)) {
            Ok(()) => {
                self.dirty = false;
                self.frames += 1;
                true
            }
            Err(_) => false,
        }
    }

    /// Tells the window about a new pointer and copied text; what does not fit waits.
    fn send_changes(&mut self, events: &mpsc::Sender<Event>) {
        if let Some(cursor) = self.cursor.take()
            && let Err(mpsc::error::TrySendError::Full(Event::Cursor(cursor))) =
                events.try_send(Event::Cursor(cursor))
        {
            self.cursor = Some(cursor);
        }
        while !self.copied.is_empty() {
            let text = self.copied.remove(0);
            if let Err(mpsc::error::TrySendError::Full(Event::Clipboard(text))) =
                events.try_send(Event::Clipboard(text))
            {
                self.copied.insert(0, text);
                break;
            }
        }
    }

    fn stats(&self) -> Stats {
        Stats {
            received: self.counters.received(),
            sent: self.counters.sent(),
            frames: self.frames,
        }
    }
}

fn area(rect: vnc::Rect) -> Area {
    Area {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

/// The reason shown when the connection ends.
fn closed_reason(error: &VncError) -> String {
    match error {
        VncError::ClientNotRunning => "The server closed the connection.".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_client_is_told_in_plain_words() {
        assert_eq!(
            closed_reason(&VncError::ClientNotRunning),
            "The server closed the connection."
        );
    }

    #[test]
    fn other_errors_keep_their_message() {
        assert!(closed_reason(&VncError::General("boom".to_string())).contains("boom"));
    }
}
