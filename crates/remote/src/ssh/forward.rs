//! Local port forwards: listen on this machine, relay each client through the
//! SSH connection to a host the server can reach.

use std::net::{Ipv4Addr, SocketAddr};

use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::{JoinHandle, JoinSet};

use super::{Connection, Error, Result};

/// A forward that is listening. Dropping it closes the port and every client
/// still connected through it.
pub struct Forwarding {
    local_port: u16,
    task: Option<JoinHandle<()>>,
    /// For a remote forward: what to tell the server when this is dropped.
    remote: Option<(Connection, tokio::runtime::Handle)>,
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
        if let Some(task) = &self.task {
            task.abort();
        }
        if let Some((connection, runtime)) = self.remote.take() {
            let port = u32::from(self.local_port);
            if let Ok(mut details) = connection.0.details.lock() {
                details.remote_targets.remove(&port);
            }
            runtime.spawn(async move {
                let _ = connection
                    .handle()
                    .cancel_tcpip_forward("127.0.0.1", port)
                    .await;
            });
        }
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
        Ok(Forwarding {
            local_port,
            task: Some(task),
            remote: None,
        })
    }

    /// Makes the server listen on its `127.0.0.1:remote_port` and sends every client to
    /// `host:port` as seen from this machine, like `ssh -R`. Port 0 lets the server pick.
    pub async fn forward_remote(
        &self,
        remote_port: u16,
        host: String,
        port: u16,
    ) -> Result<Forwarding> {
        let bound = self
            .handle()
            .tcpip_forward("127.0.0.1", u32::from(remote_port))
            .await
            .map_err(|_| Error::RemoteForwardRefused { port: remote_port })?;
        // The server says which port it took when asked for 0.
        let bound = if remote_port == 0 {
            bound
        } else {
            u32::from(remote_port)
        };
        if let Ok(mut details) = self.0.details.lock() {
            details.remote_targets.insert(bound, (host, port));
        }
        Ok(Forwarding {
            local_port: u16::try_from(bound).unwrap_or(remote_port),
            task: None,
            remote: Some((self.clone(), tokio::runtime::Handle::current())),
        })
    }

    /// Listens on `127.0.0.1:local_port` as a SOCKS5 proxy (no sign-in, `CONNECT` only) whose
    /// connections leave from the server, like `ssh -D`.
    pub async fn forward_dynamic(&self, local_port: u16) -> Result<Forwarding> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, local_port))
            .await
            .map_err(|error| Error::ListenFailed {
                port: local_port,
                reason: error.to_string(),
            })?;
        let local_port = listener
            .local_addr()
            .map_or(local_port, |address| address.port());
        let task = tokio::spawn(accept_socks(listener, self.clone()));
        Ok(Forwarding {
            local_port,
            task: Some(task),
            remote: None,
        })
    }
}

async fn accept_socks(listener: TcpListener, connection: Connection) {
    let mut clients = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, peer)) = accepted else { return };
                clients.spawn(socks_client(stream, peer, connection.clone()));
            }
            Some(_) = clients.join_next() => {}
        }
    }
}

/// Where a SOCKS5 client wants to go.
async fn socks_request(stream: &mut TcpStream) -> std::io::Result<(String, u16)> {
    let invalid =
        |reason: &'static str| std::io::Error::new(std::io::ErrorKind::InvalidData, reason);
    // Greeting: version, number of methods, the methods. Only "no sign-in" (0) is offered back.
    let mut head = [0u8; 2];
    stream.read_exact(&mut head).await?;
    if head[0] != 5 {
        return Err(invalid("not SOCKS5"));
    }
    let mut methods = vec![0u8; usize::from(head[1])];
    stream.read_exact(&mut methods).await?;
    if !methods.contains(&0) {
        stream.write_all(&[5, 0xFF]).await?;
        return Err(invalid("no usable method"));
    }
    stream.write_all(&[5, 0]).await?;
    // Request: version, command, reserved, address type.
    let mut request = [0u8; 4];
    stream.read_exact(&mut request).await?;
    if request[1] != 1 {
        // 7: command not supported.
        stream.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        return Err(invalid("only CONNECT"));
    }
    let host = match request[3] {
        1 => {
            let mut octets = [0u8; 4];
            stream.read_exact(&mut octets).await?;
            std::net::Ipv4Addr::from(octets).to_string()
        }
        3 => {
            let mut length = [0u8; 1];
            stream.read_exact(&mut length).await?;
            let mut name = vec![0u8; usize::from(length[0])];
            stream.read_exact(&mut name).await?;
            String::from_utf8(name).map_err(|_| invalid("bad host name"))?
        }
        4 => {
            let mut octets = [0u8; 16];
            stream.read_exact(&mut octets).await?;
            std::net::Ipv6Addr::from(octets).to_string()
        }
        _ => {
            // 8: address type not supported.
            stream.write_all(&[5, 8, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            return Err(invalid("bad address type"));
        }
    };
    let mut port = [0u8; 2];
    stream.read_exact(&mut port).await?;
    Ok((host, u16::from_be_bytes(port)))
}

/// Carries one SOCKS5 client through the server.
async fn socks_client(mut stream: TcpStream, peer: SocketAddr, connection: Connection) {
    let Ok((host, port)) = socks_request(&mut stream).await else {
        return;
    };
    let opened = connection
        .handle()
        .channel_open_direct_tcpip(
            host,
            u32::from(port),
            peer.ip().to_string(),
            u32::from(peer.port()),
        )
        .await;
    let Ok(channel) = opened else {
        // 5: connection refused.
        let _ = stream.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await;
        return;
    };
    if stream
        .write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])
        .await
        .is_err()
    {
        return;
    }
    let _ = copy_bidirectional(&mut stream, &mut channel.into_stream()).await;
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
