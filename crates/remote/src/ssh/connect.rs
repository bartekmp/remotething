//! Connecting and signing in.

use std::sync::Arc;
use std::time::Duration;

use remotething_secrets::KnownHosts;
use russh::client;
use russh::keys::PrivateKeyWithHashAlg;
use tokio::net::TcpStream;

use super::host_key::{Handler, Refusal};
use super::{Auth, Endpoint, Error, Result, Shell, TerminalSize, shell};

/// For reaching the server, the handshake and the sign-in together.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Keeps firewalls from dropping an idle session.
const KEEPALIVE: Duration = Duration::from_secs(30);
/// What the remote programs are told the terminal is.
const TERMINAL_TYPE: &str = "xterm-256color";

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
    let attempt = connect(endpoint, auth, known, size);
    match tokio::time::timeout(CONNECT_TIMEOUT, attempt).await {
        Ok(result) => result,
        Err(_) => Err(Error::TimedOut(CONNECT_TIMEOUT.as_secs())),
    }
}

async fn connect(
    endpoint: &Endpoint,
    auth: &Auth,
    known: KnownHosts,
    size: TerminalSize,
) -> Result<Shell> {
    let address = (endpoint.host.as_str(), endpoint.port);
    let tcp = TcpStream::connect(address)
        .await
        .map_err(|error| Error::Unreachable {
            host: endpoint.host.clone(),
            port: endpoint.port,
            reason: error.to_string(),
        })?;

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
    let mut session = match client::connect_stream(config, tcp, handler).await {
        Ok(session) => session,
        Err(error) => {
            // A refused host key surfaces as a generic error; report the real reason.
            let refused = refusal.lock().ok().and_then(|mut refusal| refusal.take());
            return Err(refused.unwrap_or_else(|| error.into()));
        }
    };

    sign_in(&mut session, auth).await?;

    let channel = session.channel_open_session().await?;
    let (columns, lines) = (u32::from(size.columns), u32::from(size.lines));
    channel
        .request_pty(false, TERMINAL_TYPE, columns, lines, 0, 0, &[])
        .await?;
    channel.request_shell(false).await?;

    Ok(shell::start(session, channel))
}

async fn sign_in(session: &mut client::Handle<Handler>, auth: &Auth) -> Result<()> {
    let outcome = match auth {
        Auth::Password { username, password } => {
            session
                .authenticate_password(username, password.expose())
                .await?
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
