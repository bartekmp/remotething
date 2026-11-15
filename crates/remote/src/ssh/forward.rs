//! Local port forwards: listen on this machine, relay each client through the
//! SSH connection to a host the server can reach.

use std::net::{Ipv4Addr, SocketAddr};

use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::{JoinHandle, JoinSet};

use super::{Connection, Error, Result};

/// A forward that is listening. Dropping it closes the port and every client
/// still connected through it.
pub struct Forwarding {
    local_port: u16,
    task: JoinHandle<()>,
}

impl Forwarding {
    /// The port listened on, on this machine's loopback address.
    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}

impl Drop for Forwarding {
    fn drop(&mut self) {
        // Aborting the accept task drops its `JoinSet`, which stops the clients.
        self.task.abort();
    }
}

impl Connection {
    /// Listens on `127.0.0.1:local_port` and sends every client to `host:port`
    /// as seen from the server. Only this machine can connect.
    pub async fn forward_local(
        &self,
        local_port: u16,
        host: String,
        port: u16,
    ) -> Result<Forwarding> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, local_port))
            .await
            .map_err(|error| Error::ListenFailed {
                port: local_port,
                reason: error.to_string(),
            })?;
        // Port 0 asks the system for a free one, which only tests do.
        let local_port = listener
            .local_addr()
            .map_or(local_port, |address| address.port());
        let task = tokio::spawn(accept(listener, self.clone(), host, port));
        Ok(Forwarding { local_port, task })
    }
}

async fn accept(listener: TcpListener, connection: Connection, host: String, port: u16) {
    let mut clients = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, peer)) = accepted else { return };
                clients.spawn(relay(stream, peer, connection.clone(), host.clone(), port));
            }
            // Reap finished clients so the set does not grow.
            Some(_) = clients.join_next() => {}
        }
    }
}

/// Carries one client until either side closes. A refused target just closes
/// the client, like a closed port would.
async fn relay(
    mut stream: TcpStream,
    peer: SocketAddr,
    connection: Connection,
    host: String,
    port: u16,
) {
    let opened = connection
        .handle()
        .channel_open_direct_tcpip(
            host,
            u32::from(port),
            peer.ip().to_string(),
            u32::from(peer.port()),
        )
        .await;
    let Ok(channel) = opened else { return };
    let _ = copy_bidirectional(&mut stream, &mut channel.into_stream()).await;
}
