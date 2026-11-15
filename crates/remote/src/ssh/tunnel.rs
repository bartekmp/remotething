//! A byte stream to a host that only an SSH server can reach.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use remotething_secrets::KnownHosts;
use russh::client::{self, Msg};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::connect::connect_hops;
use super::host_key::Handler;
use super::{Endpoint, Error, Hop, Result};

/// For reaching every server on the way, signing in to each and opening the channel.
const TUNNEL_TIMEOUT: Duration = Duration::from_secs(30);
/// What the last server is told the relayed connection comes from.
const LOCAL_ORIGIN: &str = "127.0.0.1";

/// A connection to `host:port` through SSH. It stays open while this is held.
pub struct Tunnel {
    stream: russh::ChannelStream<Msg>,
    /// The connections the stream runs over.
    _links: Vec<client::Handle<Handler>>,
}

/// Connects through `hops` in order (the first directly, each next one from the
/// one before it) and opens a channel from the last to `target`.
///
/// Every server on the way has its key checked against `known` and is signed
/// in to with its own credentials, as for [`super::open_shell_through`].
pub async fn open_tunnel(hops: &[Hop], target: &Endpoint, known: KnownHosts) -> Result<Tunnel> {
    match tokio::time::timeout(TUNNEL_TIMEOUT, open(hops, target, known)).await {
        Ok(result) => result,
        Err(_) => Err(Error::TimedOut(TUNNEL_TIMEOUT.as_secs())),
    }
}

async fn open(hops: &[Hop], target: &Endpoint, known: KnownHosts) -> Result<Tunnel> {
    let Some(last) = hops.last() else {
        return Err(Error::Protocol(
            "a tunnel needs at least one SSH connection".to_string(),
        ));
    };
    let links = connect_hops(hops, &known).await?;
    let relay = links.last().expect("one link for every hop");
    let channel = relay
        .channel_open_direct_tcpip(target.host.clone(), u32::from(target.port), LOCAL_ORIGIN, 0)
        .await
        .map_err(|error| match error {
            russh::Error::ChannelOpenFailure(
                russh::ChannelOpenFailure::AdministrativelyProhibited,
            ) => Error::ForwardingRefused {
                host: last.endpoint.host.clone(),
            },
            other => Error::Unreachable {
                host: target.host.clone(),
                port: target.port,
                reason: other.to_string(),
            },
        })?;
    Ok(Tunnel {
        stream: channel.into_stream(),
        _links: links,
    })
}

impl AsyncRead for Tunnel {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for Tunnel {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
