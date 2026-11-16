//! Tests of `tight.rs`: the server and the library are played by hand.

use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream, duplex};

use super::*;

/// A fake server and the stream the library gets, connected through the relay.
fn pair() -> (DuplexStream, DuplexStream) {
    let (server, ours) = duplex(4096);
    (ours, relay(server))
}

async fn capability(server: &mut DuplexStream, code: u32) {
    server.write_u32(code).await.unwrap();
    server.write_all(&[0u8; 12]).await.unwrap();
}

#[tokio::test]
async fn a_tight_server_looks_like_a_password_server_to_the_library() {
    let (mut server, mut library) = pair();
    server.write_all(b"RFB 003.008\n").await.unwrap();
    let mut version = [0u8; 12];
    library.read_exact(&mut version).await.unwrap();
    assert_eq!(&version, b"RFB 003.008\n");
    library.write_all(b"RFB 003.008\n").await.unwrap();
    server.read_exact(&mut version).await.unwrap();

    // The server offers Tight only; Tight offers no tunnel and a password or nothing.
    server.write_all(&[1, 16]).await.unwrap();
    server.write_u32(0).await.unwrap();
    server.write_u32(2).await.unwrap();
    capability(&mut server, 1).await;
    capability(&mut server, 2).await;

    // The relay chose Tight, then the password, and told the library the server asks for one.
    assert_eq!(server.read_u8().await.unwrap(), 16);
    assert_eq!(server.read_u32().await.unwrap(), 2);
    let mut offered = [0u8; 2];
    library.read_exact(&mut offered).await.unwrap();
    assert_eq!(offered, [1, 2]);

    // The library's own choice is not passed on; what follows goes through both ways.
    library.write_u8(2).await.unwrap();
    server.write_all(&[7; 16]).await.unwrap();
    let mut challenge = [0u8; 16];
    library.read_exact(&mut challenge).await.unwrap();
    assert_eq!(challenge, [7; 16]);
    library.write_all(&[9; 16]).await.unwrap();
    let mut answer = [0u8; 16];
    server.read_exact(&mut answer).await.unwrap();
    assert_eq!(answer, [9; 16]);
}

#[tokio::test]
async fn tunnel_types_are_answered_with_no_tunnel() {
    let (mut server, mut library) = pair();
    server.write_all(b"RFB 003.008\n").await.unwrap();
    let mut version = [0u8; 12];
    library.read_exact(&mut version).await.unwrap();
    library.write_all(&version).await.unwrap();
    server.read_exact(&mut version).await.unwrap();

    server.write_all(&[1, 16]).await.unwrap();
    server.write_u32(1).await.unwrap();
    capability(&mut server, 0).await;
    server.write_u32(1).await.unwrap();
    capability(&mut server, 2).await;

    assert_eq!(server.read_u8().await.unwrap(), 16);
    assert_eq!(server.read_u32().await.unwrap(), 0, "no tunnel");
    assert_eq!(server.read_u32().await.unwrap(), 2, "the password");
}

#[tokio::test]
async fn a_server_with_no_way_of_signing_in_that_we_know_is_reported_to_the_library() {
    let (mut server, mut library) = pair();
    server.write_all(b"RFB 003.008\n").await.unwrap();
    let mut version = [0u8; 12];
    library.read_exact(&mut version).await.unwrap();
    library.write_all(&version).await.unwrap();
    server.read_exact(&mut version).await.unwrap();

    server.write_all(&[1, 16]).await.unwrap();
    server.write_u32(0).await.unwrap();
    server.write_u32(1).await.unwrap();
    server.write_u32(1133).await.unwrap();
    server.write_all(b"TGHTSRP6DUAL").await.unwrap();

    // The library hears "no security types" and the reason, as from a refusing server.
    assert_eq!(library.read_u8().await.unwrap(), 0);
    let length = library.read_u32().await.unwrap() as usize;
    let mut reason = vec![0u8; length];
    library.read_exact(&mut reason).await.unwrap();
    let reason = String::from_utf8(reason).unwrap();
    assert!(reason.contains("not supported"), "{reason}");
    assert!(reason.contains("1133 TGHTSRP6DUAL"), "{reason}");
    assert!(reason.contains("password authentication"), "{reason}");
}

#[tokio::test]
async fn an_ordinary_password_server_is_passed_through_untouched() {
    let (mut server, mut library) = pair();
    server.write_all(b"RFB 003.008\n").await.unwrap();
    let mut version = [0u8; 12];
    library.read_exact(&mut version).await.unwrap();
    library.write_all(&version).await.unwrap();
    server.read_exact(&mut version).await.unwrap();

    server.write_all(&[2, 1, 2]).await.unwrap();
    let mut offered = [0u8; 3];
    library.read_exact(&mut offered).await.unwrap();
    assert_eq!(offered, [2, 1, 2]);
    library.write_u8(2).await.unwrap();

    // The library's choice reaches the server.
    assert_eq!(server.read_u8().await.unwrap(), 2);
}

#[tokio::test]
async fn a_server_that_refuses_has_its_reason_passed_on() {
    let (mut server, mut library) = pair();
    server.write_all(b"RFB 003.008\n").await.unwrap();
    let mut version = [0u8; 12];
    library.read_exact(&mut version).await.unwrap();
    library.write_all(&version).await.unwrap();
    server.read_exact(&mut version).await.unwrap();

    server.write_u8(0).await.unwrap();
    server.write_u32(4).await.unwrap();
    server.write_all(b"busy").await.unwrap();

    assert_eq!(library.read_u8().await.unwrap(), 0);
    assert_eq!(library.read_u32().await.unwrap(), 4);
    let mut reason = [0u8; 4];
    library.read_exact(&mut reason).await.unwrap();
    assert_eq!(&reason, b"busy");
}

#[test]
fn tight_is_only_used_when_it_is_the_only_way() {
    assert!(only_tight(&[16]));
    assert!(!only_tight(&[2, 16]));
    assert!(!only_tight(&[1, 16]));
    assert!(!only_tight(&[2]));
}
