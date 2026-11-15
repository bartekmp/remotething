//! Opening a session and running it on its own thread.

use std::io::ErrorKind;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use ironrdp_connector::sspi::generator::NetworkRequest;
use ironrdp_connector::sspi::network_client::NetworkClient;
use ironrdp_connector::{ClientConnector, ConnectionResult};
use ironrdp_graphics::image_processing::PixelFormat;
use ironrdp_session::image::DecodedImage;
use ironrdp_session::{ActiveStage, ActiveStageBuilder, ActiveStageOutput};
use remotething_secrets::{KnownHosts, Secret};
use tokio::sync::mpsc;

use crate::error::{Error, Result};
use crate::input::{self, Input};
use crate::stats::{Counter, Stats};
use crate::{settings, tls};

/// The most frames sent to the window per second.
const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Short, so input and a pending frame go out while the server is quiet.
const IDLE_READ_TIMEOUT: Duration = Duration::from_millis(20);
/// How many frames or inputs may wait before the sender has to catch up.
const QUEUE: usize = 4;
const INPUT_QUEUE: usize = 256;

/// Where to connect and who to sign in as.
pub struct Target {
    /// The host name or address.
    pub host: String,
    /// The TCP port, usually 3389.
    pub port: u16,
    /// The account name.
    pub username: String,
    /// Its password.
    pub password: Secret,
    /// The Windows domain, if the account is not local.
    pub domain: Option<String>,
    /// The width of the remote desktop in pixels.
    pub width: u16,
    /// The height of the remote desktop in pixels.
    pub height: u16,
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

/// Facts about a connection that do not change while it lasts.
#[derive(Debug, Clone)]
pub struct Info {
    /// The server's address and port as connected to.
    pub peer: SocketAddr,
    /// How the picture is sent, as far as this client asked for it.
    pub codec: &'static str,
    /// How the connection is secured.
    pub security: &'static str,
}

/// What a session tells the window.
pub enum Event {
    /// Running totals, about once per second.
    Stats(Stats),
    /// The screen changed. Only the newest frame matters.
    Frame(Frame),
    /// The session ended, with the reason when it was not the user's doing.
    Closed(Option<String>),
}

/// A running session. It ends when this is dropped.
pub struct Session {
    /// What is known about the connection.
    pub info: Info,
    /// Frames and the end of the session, in order.
    pub events: mpsc::Receiver<Event>,
    /// Where the user's input goes.
    pub input: mpsc::Sender<Input>,
}

/// Connects, signs in and starts the session thread. This blocks until the
/// desktop is up, so call it off the UI thread.
///
/// The server's certificate must already be pinned in `known`; otherwise the
/// attempt stops before any credential is sent and the error carries it.
pub fn connect(target: Target, known: &KnownHosts) -> Result<Session> {
    let (connection, framed, peer) = open(&target, known)?;
    let info = Info {
        peer,
        codec: settings::CODEC,
        security: settings::SECURITY,
    };
    let (events_tx, events) = mpsc::channel(QUEUE);
    let (input, input_rx) = mpsc::channel(INPUT_QUEUE);
    std::thread::spawn(move || {
        let reason = match run(connection, framed, &events_tx, input_rx) {
            Ok(()) => None,
            Err(error) => Some(error.to_string()),
        };
        let _ = events_tx.blocking_send(Event::Closed(reason));
    });
    Ok(Session {
        info,
        events,
        input,
    })
}

type TlsFramed = ironrdp_blocking::Framed<tls::Stream>;

fn protocol(error: impl std::fmt::Display) -> Error {
    Error::Protocol(error.to_string())
}

fn open(target: &Target, known: &KnownHosts) -> Result<(ConnectionResult, TlsFramed, SocketAddr)> {
    let address = (target.host.as_str(), target.port);
    let tcp = TcpStream::connect(address).map_err(|error| Error::Unreachable {
        host: target.host.clone(),
        port: target.port,
        reason: error.to_string(),
    })?;
    tcp.set_read_timeout(Some(CONNECT_TIMEOUT))
        .map_err(protocol)?;
    // A second handle to the same socket, to shorten the timeout once connected.
    let socket = tcp.try_clone().map_err(protocol)?;
    let client_addr = tcp.local_addr().map_err(protocol)?;
    let peer = tcp.peer_addr().map_err(protocol)?;

    let mut framed = ironrdp_blocking::Framed::new(tcp);
    let mut connector = ClientConnector::new(settings::connector_config(target), client_addr);
    let should_upgrade =
        ironrdp_blocking::connect_begin(&mut framed, &mut connector).map_err(protocol)?;
    let tcp = framed.into_inner_no_leftover();

    let (stream, certificate, public_key) = tls::upgrade(tcp, &target.host)?;
    // Nothing secret has been sent yet; the credentials go out in CredSSP below.
    tls::check(known, &target.host, target.port, &certificate)?;

    let upgraded = ironrdp_blocking::mark_as_upgraded(should_upgrade, &mut connector);
    let mut framed = ironrdp_blocking::Framed::new(stream);
    let connection = ironrdp_blocking::connect_finalize(
        upgraded,
        connector,
        &mut framed,
        &mut NoKerberos,
        target.host.clone().into(),
        public_key,
        None,
    )
    .map_err(sign_in_error)?
    // Following a redirection to another server is not supported.
    .into_connected()
    .map_err(protocol)?;

    socket
        .set_read_timeout(Some(IDLE_READ_TIMEOUT))
        .map_err(protocol)?;
    Ok((connection, framed, peer))
}

/// A refusal of the credentials is told apart from other failures.
fn sign_in_error(error: ironrdp_connector::ConnectorError) -> Error {
    use ironrdp_connector::ConnectorErrorKind;
    match error.kind() {
        ConnectorErrorKind::AccessDenied | ConnectorErrorKind::Credssp(_) => Error::AuthRefused,
        _ => protocol(error),
    }
}

/// CredSSP only needs the network for Kerberos. NTLM, used here, does not.
struct NoKerberos;

impl NetworkClient for NoKerberos {
    fn send(&self, _request: &NetworkRequest) -> ironrdp_connector::sspi::Result<Vec<u8>> {
        Err(ironrdp_connector::sspi::Error::new(
            ironrdp_connector::sspi::ErrorKind::NoAuthenticatingAuthority,
            "Kerberos is not supported",
        ))
    }
}

fn run(
    connection: ConnectionResult,
    mut framed: TlsFramed,
    events: &mpsc::Sender<Event>,
    mut input: mpsc::Receiver<Input>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut image = DecodedImage::new(
        PixelFormat::RgbA32,
        connection.desktop_size.width,
        connection.desktop_size.height,
    );
    let mut stage = ActiveStageBuilder {
        static_channels: connection.static_channels,
        user_channel_id: connection.user_channel_id,
        io_channel_id: connection.io_channel_id,
        message_channel_id: connection.message_channel_id,
        share_id: connection.share_id,
        compression_type: connection.compression_type,
        enable_server_pointer: connection.enable_server_pointer,
        pointer_software_rendering: connection.pointer_software_rendering,
    }
    .build();

    let mut dirty = false;
    let mut last_sent = Instant::now();
    let mut counter = Counter::new();
    loop {
        match framed.read_pdu() {
            Ok((action, payload)) => {
                counter.received(payload.len());
                let received_at = framed.last_read_at();
                let outputs = stage.process(&mut image, action, &payload, received_at)?;
                if handle(outputs, &mut framed, &mut dirty)? {
                    return Ok(());
                }
            }
            // The read timeout lets input and a pending frame go out while the server is quiet.
            Err(error) if is_timeout(&error) => {}
            Err(error) => return Err(error.into()),
        }

        if !forward_input(&mut input, &mut stage, &mut image, &mut framed, &mut dirty)? {
            // The window dropped the session.
            return Ok(());
        }
        if dirty && last_sent.elapsed() >= FRAME_INTERVAL {
            // A full queue means the window is behind; this frame is skipped and a later one follows.
            match events.try_send(Event::Frame(copy_frame(&image))) {
                Ok(()) => {
                    dirty = false;
                    last_sent = Instant::now();
                    counter.frame_sent();
                }
                Err(mpsc::error::TrySendError::Full(_)) => {}
                Err(mpsc::error::TrySendError::Closed(_)) => return Ok(()),
            }
        }
        if let Some(totals) = counter.due() {
            // A full queue only delays the numbers.
            let _ = events.try_send(Event::Stats(totals));
        }
    }
}

/// Writes what the session wants sent. True when the server ended the session.
fn handle(
    outputs: Vec<ActiveStageOutput>,
    framed: &mut TlsFramed,
    dirty: &mut bool,
) -> std::io::Result<bool> {
    for output in outputs {
        match output {
            ActiveStageOutput::ResponseFrame(response) => framed.write_all(&response)?,
            ActiveStageOutput::GraphicsUpdate(_) => *dirty = true,
            ActiveStageOutput::Terminate(_) => return Ok(true),
            _ => {}
        }
    }
    Ok(false)
}

/// Sends the input that is waiting. False when the window is gone.
fn forward_input(
    input: &mut mpsc::Receiver<Input>,
    stage: &mut ActiveStage,
    image: &mut DecodedImage,
    framed: &mut TlsFramed,
    dirty: &mut bool,
) -> std::result::Result<bool, Box<dyn std::error::Error>> {
    let mut events = Vec::new();
    loop {
        match input.try_recv() {
            Ok(one) => events.extend(input::to_events(one)),
            Err(mpsc::error::TryRecvError::Empty) => break,
            Err(mpsc::error::TryRecvError::Disconnected) => return Ok(false),
        }
    }
    if !events.is_empty() {
        let outputs = stage.process_fastpath_input(image, &events)?;
        handle(outputs, framed, dirty)?;
    }
    Ok(true)
}

fn is_timeout(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

fn copy_frame(image: &DecodedImage) -> Frame {
    let mut rgba = image.data().to_vec();
    // The decoder leaves alpha undefined; the window needs opaque pixels.
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    Frame {
        width: u32::from(image.width()),
        height: u32::from(image.height()),
        rgba,
    }
}
