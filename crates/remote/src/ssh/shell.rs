//! The task that carries bytes between a shell channel and the two queues of a [`Shell`].

use std::sync::Arc;

use russh::client::{self, Msg};
use russh::{Channel, ChannelMsg};
use tokio::sync::mpsc;

use super::host_key::{Handler, Learned};
use super::{Details, Result, Shell, ShellEvent, ShellInput};

/// The byte stream of an SFTP channel.
pub type SftpStream = russh::ChannelStream<Msg>;

/// An open SSH connection. It stays open while a shell or a copy of this handle uses it.
#[derive(Clone)]
pub struct Connection(Arc<Links>);

/// The connection to the target and, kept open underneath it, those to the jump hosts.
struct Links {
    target: client::Handle<Handler>,
    _tunnel: Vec<client::Handle<Handler>>,
    details: Learned,
}

impl Connection {
    /// What is known about this connection.
    pub fn details(&self) -> Details {
        self.0
            .details
            .lock()
            .map(|details| details.clone())
            .unwrap_or_default()
    }

    /// The client handle of the target, for opening more channels.
    pub(super) fn handle(&self) -> &client::Handle<Handler> {
        &self.0.target
    }

    /// Opens a channel on this connection and starts the server's SFTP service on it.
    pub async fn open_sftp(&self) -> Result<SftpStream> {
        let channel = self.0.target.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        Ok(channel.into_stream())
    }
}

/// How many events or inputs may wait before the sender is slowed down.
const QUEUE: usize = 64;

/// Starts the task and returns its two ends.
pub(super) fn start(
    session: client::Handle<Handler>,
    tunnel: Vec<client::Handle<Handler>>,
    channel: Channel<Msg>,
    details: Learned,
) -> Shell {
    let (events_tx, events) = mpsc::channel(QUEUE);
    let (input, input_rx) = mpsc::channel(QUEUE);
    let connection = connection(session, tunnel, details);
    tokio::spawn(run(connection.clone(), channel, events_tx, input_rx));
    Shell {
        events,
        input,
        connection,
    }
}

/// A connection with no shell on it.
pub(super) fn connection(
    session: client::Handle<Handler>,
    tunnel: Vec<client::Handle<Handler>>,
    details: Learned,
) -> Connection {
    Connection(Arc::new(Links {
        target: session,
        _tunnel: tunnel,
        details,
    }))
}

async fn run(
    // Held so the connection lives as long as the shell does.
    _connection: Connection,
    mut channel: Channel<Msg>,
    events: mpsc::Sender<ShellEvent>,
    mut input: mpsc::Receiver<ShellInput>,
) {
    let mut exit_status = None;
    loop {
        tokio::select! {
            typed = input.recv() => {
                let sent = match typed {
                    Some(ShellInput::Data(bytes)) => channel.data(&bytes[..]).await,
                    Some(ShellInput::Resize(size)) => {
                        let (columns, lines) = (u32::from(size.columns), u32::from(size.lines));
                        channel.window_change(columns, lines, 0, 0).await
                    }
                    // The local side is gone: nobody is left to tell.
                    None => return,
                };
                if sent.is_err() {
                    break;
                }
            }
            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) => {
                        if events.send(ShellEvent::Output(data.to_vec())).await.is_err() {
                            return;
                        }
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        // Standard error of the shell belongs on the same screen.
                        if events.send(ShellEvent::Output(data.to_vec())).await.is_err() {
                            return;
                        }
                    }
                    Some(ChannelMsg::ExitStatus { exit_status: status }) => {
                        exit_status = Some(status);
                    }
                    Some(ChannelMsg::Close) | None => break,
                    Some(_) => {}
                }
            }
        }
    }
    let _ = events.send(ShellEvent::Closed(exit_status)).await;
}
