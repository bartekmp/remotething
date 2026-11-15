//! Connecting and signing in.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use remotething_secrets::KnownHosts;
use russh::client;
use russh::keys::PrivateKeyWithHashAlg;
use russh::keys::agent::client::AgentClient;
use tokio::net::TcpStream;

use super::host_key::{Handler, Refusal};
use super::{Auth, Endpoint, Error, Hop, Result, Shell, TerminalSize, shell};

/// For reaching the server, the handshake and the sign-in together.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Keeps firewalls from dropping an idle session.
const KEEPALIVE: Duration = Duration::from_secs(30);
/// What the remote programs are told the terminal is.
const TERMINAL_TYPE: &str = "xterm-256color";
/// What a jump host is told the relayed connection comes from.
const LOCAL_ORIGIN: &str = "127.0.0.1";

/// Connects to `endpoint`, signs in and starts a shell in a terminal of `size`.
///
/// The server's key must already be in `known`; otherwise the attempt stops
/// before any credential is sent and the error carries the key to show.
pub async fn open_shell(
    endpoint: &Endpoint,
    auth: &Auth,
    known: KnownHosts,
    size: TerminalSize,
) -> Result<Shell> {
    open_shell_through(&[], endpoint, auth, known, size).await
}

/// Like [`open_shell`], reaching the target through `jumps` in order: the
/// first is connected to directly, each next one from the one before it.
///
/// Every server on the way has its key checked against `known` and is signed
/// in to with its own credentials. The connections to the jump hosts stay
/// open for as long as the shell's connection does.
pub async fn open_shell_through(
    jumps: &[Hop],
    endpoint: &Endpoint,
    auth: &Auth,
    known: KnownHosts,
    size: TerminalSize,
) -> Result<Shell> {
    let attempt = connect(jumps, endpoint, auth, known, size);
    match tokio::time::timeout(CONNECT_TIMEOUT, attempt).await {
        Ok(result) => result,
        Err(_) => Err(Error::TimedOut(CONNECT_TIMEOUT.as_secs())),
    }
}

async fn connect(
    jumps: &[Hop],
    endpoint: &Endpoint,
    auth: &Auth,
    known: KnownHosts,
    size: TerminalSize,
) -> Result<Shell> {
    // The connections to the jump hosts, first one first.
    let mut tunnel: Vec<client::Handle<Handler>> = Vec::new();
    for (index, hop) in jumps.iter().enumerate() {
        let through = tunnel.last();
        let session = establish(through, &hop.endpoint, &hop.auth, known.clone())
            .await
            .map_err(|error| {
                // A refused relay is the doing of the host before this one.
                let relay = index.checked_sub(1).map(|before| &jumps[before].endpoint);
                on_jump(&hop.endpoint, relay, error)
            })?;
        debug_assert_eq!(tunnel.len(), index);
        tunnel.push(session);
    }
    let session = establish(tunnel.last(), endpoint, auth, known)
        .await
        .map_err(|error| match (jumps.last(), error) {
            (Some(last), Error::ForwardingRefused { .. }) => Error::ForwardingRefused {
                host: last.endpoint.host.clone(),
            },
            (_, error) => error,
        })?;

    let channel = session.channel_open_session().await?;
    let (columns, lines) = (u32::from(size.columns), u32::from(size.lines));
    channel
        .request_pty(false, TERMINAL_TYPE, columns, lines, 0, 0, &[])
        .await?;
    channel.request_shell(false).await?;

    Ok(shell::start(session, tunnel, channel))
}

/// Says which jump host a failure belongs to. A host-key question already
/// names its host and must reach the user as it is.
///
/// `relay` is the jump host before this one, which carried the connection.
fn on_jump(endpoint: &Endpoint, relay: Option<&Endpoint>, error: Error) -> Error {
    match (error, relay) {
        (error @ (Error::UnknownHostKey(_) | Error::HostKeyChanged(_)), _) => error,
        (Error::ForwardingRefused { .. }, Some(relay)) => Error::ForwardingRefused {
            host: relay.host.clone(),
        },
        (other, _) => Error::Jump {
            host: endpoint.host.clone(),
            reason: Box::new(other),
        },
    }
}

/// Reaches `endpoint` (directly, or from the server `through`), checks its
/// key and signs in.
async fn establish(
    through: Option<&client::Handle<Handler>>,
    endpoint: &Endpoint,
    auth: &Auth,
    known: KnownHosts,
) -> Result<client::Handle<Handler>> {
    let config = Arc::new(client::Config {
        keepalive_interval: Some(KEEPALIVE),
        ..client::Config::default()
    });
    let refusal = Refusal::default();
    let handler = Handler {
        endpoint: endpoint.clone(),
        known,
        refusal: Arc::clone(&refusal),
    };

    let handshake = match through {
        None => {
            let address = (endpoint.host.as_str(), endpoint.port);
            let tcp = TcpStream::connect(address)
                .await
                .map_err(|error| unreachable(endpoint, error))?;
            client::connect_stream(config, tcp, handler).await
        }
        Some(jump) => {
            // The jump host opens the TCP connection for us and relays it.
            let channel = jump
                .channel_open_direct_tcpip(
                    endpoint.host.clone(),
                    u32::from(endpoint.port),
                    LOCAL_ORIGIN,
                    0,
                )
                .await
                .map_err(|error| match error {
                    // The refusal is the jump host's, not the target's.
                    russh::Error::ChannelOpenFailure(
                        russh::ChannelOpenFailure::AdministrativelyProhibited,
                    ) => Error::ForwardingRefused {
                        host: String::new(),
                    },
                    other => unreachable(endpoint, other),
                })?;
            client::connect_stream(config, channel.into_stream(), handler).await
        }
    };
    let mut session = match handshake {
        Ok(session) => session,
        Err(error) => {
            // A refused host key surfaces as a generic error; report the real reason.
            let refused = refusal.lock().ok().and_then(|mut refusal| refusal.take());
            return Err(refused.unwrap_or_else(|| error.into()));
        }
    };

    sign_in(&mut session, auth).await?;
    Ok(session)
}

fn unreachable(endpoint: &Endpoint, error: impl std::fmt::Display) -> Error {
    Error::Unreachable {
        host: endpoint.host.clone(),
        port: endpoint.port,
        reason: error.to_string(),
    }
}

async fn sign_in(session: &mut client::Handle<Handler>, auth: &Auth) -> Result<()> {
    let outcome = match auth {
        Auth::Password { username, password } => {
            session
                .authenticate_password(username, password.expose())
                .await?
        }
        Auth::Agent { username, socket } => {
            return sign_in_with_agent(session, username, socket.as_deref()).await;
        }
        Auth::KeyFile {
            username,
            path,
            passphrase,
        } => {
            let passphrase = passphrase.as_ref().map(|secret| secret.expose());
            let key =
                russh::keys::load_secret_key(path, passphrase).map_err(|error| match error {
                    russh::keys::Error::KeyIsEncrypted => Error::KeyLocked(path.clone()),
                    other => Error::KeyFile {
                        path: path.clone(),
                        reason: other.to_string(),
                    },
                })?;
            // RSA keys sign with the strongest hash the server announced.
            let hash = session.best_supported_rsa_hash().await?.flatten();
            let key = PrivateKeyWithHashAlg::new(Arc::new(key), hash);
            session.authenticate_publickey(username, key).await?
        }
    };

    if outcome.success() {
        Ok(())
    } else {
        Err(Error::AuthRefused)
    }
}

/// Offers the agent's keys to the server one by one; the first it accepts signs us in.
async fn sign_in_with_agent(
    session: &mut client::Handle<Handler>,
    username: &str,
    socket: Option<&Path>,
) -> Result<()> {
    #[cfg(unix)]
    {
        let client = match socket {
            Some(path) => AgentClient::connect_uds(path).await,
            None => AgentClient::connect_env().await,
        };
        let client = client.map_err(|error| Error::AgentUnavailable(error.to_string()))?;
        offer_keys(session, username, client).await
    }
    #[cfg(windows)]
    {
        const OPENSSH_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";
        let pipe = socket.map_or(OPENSSH_PIPE.as_ref(), |path| path.as_os_str());
        match AgentClient::connect_named_pipe(pipe).await {
            Ok(client) => offer_keys(session, username, client).await,
            Err(pipe_error) => {
                // Pageant is PuTTY's agent; it has no socket, so only without an explicit one.
                if socket.is_some() {
                    return Err(Error::AgentUnavailable(pipe_error.to_string()));
                }
                let client = AgentClient::connect_pageant()
                    .await
                    .map_err(|_| Error::AgentUnavailable(pipe_error.to_string()))?;
                offer_keys(session, username, client).await
            }
        }
    }
}

async fn offer_keys<S>(
    session: &mut client::Handle<Handler>,
    username: &str,
    mut agent: AgentClient<S>,
) -> Result<()>
where
    S: russh::keys::agent::client::AgentStream + Unpin + Send + 'static,
{
    let identities = agent
        .request_identities()
        .await
        .map_err(|error| Error::AgentUnavailable(error.to_string()))?;
    if identities.is_empty() {
        return Err(Error::AgentEmpty);
    }
    // RSA keys sign with the strongest hash the server announced.
    let hash = session.best_supported_rsa_hash().await?.flatten();

    for identity in identities {
        // Certificates are not supported; plain keys only.
        let russh::keys::agent::AgentIdentity::PublicKey { key, .. } = identity else {
            continue;
        };
        let outcome = session
            .authenticate_publickey_with(username, key, hash, &mut agent)
            .await
            .map_err(|error| Error::AgentUnavailable(error.to_string()))?;
        if outcome.success() {
            return Ok(());
        }
    }
    Err(Error::AuthRefused)
}
