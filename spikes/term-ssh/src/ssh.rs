//! An interactive shell over a russh channel, exposed as two byte channels.

use std::net::IpAddr;
use std::sync::Arc;

use anyhow::{Result, bail};
use russh::ChannelMsg;
use russh::client;
use russh::keys::PublicKeyOrCertificate;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

struct AcceptAnyHostKey;

impl client::Handler for AcceptAnyHostKey {
    type Error = russh::Error;

    // Spike only: the real app pins host keys. See `check_unverified_peer`.
    async fn check_server_key(
        &mut self,
        _key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// The spike does not verify who it talks to, so it only leaves the machine on request.
fn check_unverified_peer(peer: IpAddr, insecure: bool) -> Result<()> {
    if peer.is_loopback() || insecure {
        return Ok(());
    }
    bail!(
        "{peer} is not loopback and its identity is not verified; pass --insecure to connect anyway"
    )
}

/// Both ends of a remote shell: bytes to print, and bytes typed.
pub struct Shell {
    pub output: mpsc::Receiver<Vec<u8>>,
    pub input: mpsc::Sender<Vec<u8>>,
}

pub async fn open_shell(
    addr: &str,
    user: &str,
    password: &str,
    columns: u32,
    lines: u32,
    insecure: bool,
) -> Result<Shell> {
    let config = Arc::new(client::Config::default());
    let tcp = TcpStream::connect(addr).await?;
    check_unverified_peer(tcp.peer_addr()?.ip(), insecure)?;
    let mut session = client::connect_stream(config, tcp, AcceptAnyHostKey).await?;

    let auth = session.authenticate_password(user, password).await?;
    if !auth.success() {
        bail!("password authentication failed");
    }

    let mut channel = session.channel_open_session().await?;
    channel
        .request_pty(false, "xterm-256color", columns, lines, 0, 0, &[])
        .await?;
    channel.request_shell(false).await?;

    let (output_tx, output) = mpsc::channel::<Vec<u8>>(64);
    let (input, mut input_rx) = mpsc::channel::<Vec<u8>>(64);

    tokio::spawn(async move {
        // Keep the session alive for as long as the channel is in use.
        let _session = session;
        loop {
            tokio::select! {
                typed = input_rx.recv() => {
                    let Some(bytes) = typed else { break };
                    if channel.data(&bytes[..]).await.is_err() {
                        break;
                    }
                }
                message = channel.wait() => {
                    match message {
                        Some(ChannelMsg::Data { data }) => {
                            if output_tx.send(data.to_vec()).await.is_err() {
                                break;
                            }
                        }
                        Some(_) => {}
                        None => break,
                    }
                }
            }
        }
    });

    Ok(Shell { output, input })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_is_allowed_without_the_flag() {
        assert!(check_unverified_peer("127.0.0.1".parse().unwrap(), false).is_ok());
    }

    #[test]
    fn a_remote_host_is_refused_without_the_flag() {
        assert!(check_unverified_peer("192.0.2.10".parse().unwrap(), false).is_err());
    }

    #[test]
    fn a_remote_host_is_allowed_with_the_flag() {
        assert!(check_unverified_peer("192.0.2.10".parse().unwrap(), true).is_ok());
    }
}
