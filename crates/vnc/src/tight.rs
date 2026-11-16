//! Servers that offer only the Tight security type (16), such as TightVNC.
//!
//! The VNC library speaks plain password authentication but not Tight's
//! negotiation, which comes first and then ends in the same password exchange.
//! [`relay`] sits between the server and the library: it does Tight's part
//! itself and shows the library an ordinary server that asks for a password.
//! Every other server is passed through as it is.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, duplex};

/// The security type of Tight.
const TIGHT: u8 = 16;
/// Authentication by password, and none.
const AUTH_PASSWORD: u32 = 2;
const AUTH_NONE: u32 = 1;
/// "No tunnel", the only tunnelling this client does.
const NO_TUNNEL: u32 = 0;
/// A list of capabilities longer than this is not believed.
const MOST_CAPABILITIES: u32 = 64;
/// How much may wait in the pipe to the library.
const PIPE: usize = 256 * 1024;

/// Puts the relay between `server` and the returned stream, which is for the VNC library.
pub(crate) fn relay<S>(server: S) -> DuplexStream
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (library, relay_end) = duplex(PIPE);
    tokio::spawn(async move {
        let mut library = relay_end;
        let mut server = server;
        if let Err(error) = run(&mut server, &mut library).await {
            // Said the way a server says it, so the library reports it as the reason.
            let _ = refuse(&mut library, &error.to_string()).await;
        }
    });
    library
}

/// Relays the start of the conversation, then everything after it.
async fn run<S, L>(server: &mut S, library: &mut L) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
    L: AsyncRead + AsyncWrite + Unpin,
{
    // The protocol version is agreed between the two ends as it is.
    let mut version = [0u8; 12];
    server.read_exact(&mut version).await?;
    library.write_all(&version).await?;
    library.read_exact(&mut version).await?;
    server.write_all(&version).await?;

    let count = server.read_u8().await?;
    if count == 0 {
        // The server refuses and says why; the library reads it from here.
        library.write_u8(0).await?;
        tokio::io::copy(server, library).await?;
        return Ok(());
    }
    let mut types = vec![0u8; usize::from(count)];
    server.read_exact(&mut types).await?;

    if only_tight(&types) {
        server.write_u8(TIGHT).await?;
        let chosen = negotiate(server).await?;
        // The library is told the server offers this one thing, and its choice is not needed.
        library.write_all(&[1, chosen]).await?;
        let _ = library.read_u8().await?;
    } else {
        library.write_u8(count).await?;
        library.write_all(&types).await?;
        let choice = library.read_u8().await?;
        server.write_u8(choice).await?;
    }
    tokio::io::copy_bidirectional(server, library).await?;
    Ok(())
}

/// Whether Tight is the only way in this server offers.
fn only_tight(types: &[u8]) -> bool {
    types.contains(&TIGHT) && !types.iter().any(|kind| *kind == 1 || *kind == 2)
}

/// Tight's part: no tunnel, then the way of signing in. Returns the plain
/// security type that follows: 2 for a password, 1 for none.
async fn negotiate<S>(server: &mut S) -> io::Result<u8>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tunnels = read_count(server).await?;
    skip_capabilities(server, tunnels).await?;
    if tunnels > 0 {
        server.write_u32(NO_TUNNEL).await?;
    }
    let methods = read_count(server).await?;
    let mut offered = Vec::new();
    let mut described = Vec::new();
    for _ in 0..methods {
        let code = server.read_u32().await?;
        let mut vendor_and_name = [0u8; 12];
        server.read_exact(&mut vendor_and_name).await?;
        offered.push(code);
        described.push(describe(code, &vendor_and_name));
    }
    // Nothing offered means there is nothing to sign in with.
    if methods == 0 {
        return Ok(1);
    }
    let chosen = if offered.contains(&AUTH_PASSWORD) {
        AUTH_PASSWORD
    } else if offered.contains(&AUTH_NONE) {
        AUTH_NONE
    } else {
        return Err(io::Error::other(format!(
            "the server only offers a way of signing in that is not supported ({}); \
             turn on plain VNC password authentication in the server",
            described.join(", ")
        )));
    };
    server.write_u32(chosen).await?;
    Ok(if chosen == AUTH_PASSWORD { 2 } else { 1 })
}

/// A method as the server named it, such as `1133 TGHT SRP6DUAL`.
fn describe(code: u32, vendor_and_name: &[u8; 12]) -> String {
    let text = String::from_utf8_lossy(vendor_and_name);
    let text: String = text
        .chars()
        .filter(|character| character.is_ascii_graphic())
        .collect();
    format!("{code} {text}")
}

async fn read_count<S: AsyncRead + Unpin>(server: &mut S) -> io::Result<u32> {
    let count = server.read_u32().await?;
    if count > MOST_CAPABILITIES {
        return Err(io::Error::other("the server sent an unreasonable list"));
    }
    Ok(count)
}

async fn skip_capabilities<S: AsyncRead + Unpin>(server: &mut S, count: u32) -> io::Result<()> {
    let mut capability = [0u8; 16];
    for _ in 0..count {
        server.read_exact(&mut capability).await?;
    }
    Ok(())
}

/// Tells the library, the way a server does, that no security type is on offer and why.
async fn refuse<L: AsyncWrite + Unpin>(library: &mut L, reason: &str) -> io::Result<()> {
    library.write_u8(0).await?;
    library.write_u32(reason.len() as u32).await?;
    library.write_all(reason.as_bytes()).await?;
    library.flush().await
}

#[cfg(test)]
#[path = "tight_tests.rs"]
mod tests;
