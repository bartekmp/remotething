//! Opening a session and running it on its own thread.

use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use ironrdp_connector::sspi::generator::NetworkRequest;
use ironrdp_connector::sspi::network_client::NetworkClient;
use ironrdp_connector::{ClientConnector, Config, ConnectionResult};
use ironrdp_displaycontrol::client::DisplayControlClient;
use ironrdp_dvc::DrdynvcClient;
use remotething_secrets::{KnownHosts, Secret};
use tokio::sync::mpsc;

use crate::error::{Error, Result};
use crate::input::Input;
use crate::stats::Stats;
use crate::worker::Worker;
use crate::{settings, tls};

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
    let opened = open(&target, known)?;
    let info = Info {
        peer: opened.peer,
        codec: settings::CODEC,
        security: settings::SECURITY,
    };
    let (events_tx, events) = mpsc::channel(QUEUE);
    let (input, input_rx) = mpsc::channel(INPUT_QUEUE);
    std::thread::spawn(move || {
        let reason = match Worker::new(opened.connection, opened.framed, opened.config)
            .run(&events_tx, input_rx)
        {
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

pub(crate) type TlsFramed = ironrdp_blocking::Framed<tls::Stream>;

/// A signed-in connection, before the session thread takes it over.
struct Opened {
    connection: ConnectionResult,
    framed: TlsFramed,
    peer: SocketAddr,
    /// Kept to answer the server when it changes the desktop size.
    config: Config,
}

fn protocol(error: impl std::fmt::Display) -> Error {
    Error::Protocol(error.to_string())
}

fn open(target: &Target, known: &KnownHosts) -> Result<Opened> {
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
    let config = settings::connector_config(target);
    let mut connector = ClientConnector::new(config.clone(), client_addr);
    // The channel the desktop is resized over.
    connector.attach_static_channel(
        DrdynvcClient::new().with_dynamic_channel(DisplayControlClient::new(|_| Ok(Vec::new()))),
    );
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
    Ok(Opened {
        connection,
        framed,
        peer,
        config,
    })
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
