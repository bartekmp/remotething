//! `open_shell` against a small SSH server running inside the test.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use remotething_remote::ssh::{
    Auth, Endpoint, Error, Shell, ShellEvent, ShellInput, TerminalSize, open_shell,
};
use remotething_secrets::{KnownHosts, Secret};
use russh::keys::PublicKey;
use russh::server::{self, Msg, Session};
use russh::{Channel, ChannelId};
use tokio::net::TcpListener;

const USER: &str = "tester";
const PASSWORD: &str = "correct horse";
const SIZE: TerminalSize = TerminalSize {
    columns: 80,
    lines: 24,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Accepts one user by password or by the client test key, and echoes what it is sent.
struct TestServer {
    accepted_key: PublicKey,
}

impl server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        if user == USER && password == PASSWORD {
            return Ok(server::Auth::Accept);
        }
        Ok(server::Auth::reject())
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: server::VerifiedPublicKey<'_>,
    ) -> Result<server::Auth, Self::Error> {
        if user == USER && key.public_key.key_data() == self.accepted_key.key_data() {
            return Ok(server::Auth::Accept);
        }
        Ok(server::Auth::reject())
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        columns: u32,
        lines: u32,
        _pixel_width: u32,
        _pixel_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, format!("pty {term} {columns}x{lines}\n"))?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        columns: u32,
        lines: u32,
        _pixel_width: u32,
        _pixel_height: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.data(channel, format!("resized {columns}x{lines}\n"))?;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if data == b"exit\n" {
            session.exit_status_request(channel, 7)?;
            session.close(channel)?;
            return Ok(());
        }
        let mut echo = b"echo ".to_vec();
        echo.extend_from_slice(data);
        session.data(channel, echo)?;
        Ok(())
    }
}

/// Starts the server on a free local port and returns where it listens.
async fn start_server() -> Endpoint {
    let host_key = russh::keys::load_secret_key(fixture("host_key"), None).unwrap();
    let accepted_key = russh::keys::load_public_key(fixture("client_key.pub")).unwrap();
    let config = Arc::new(server::Config {
        keys: vec![host_key],
        auth_rejection_time: Duration::ZERO,
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..server::Config::default()
    });

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let handler = TestServer {
                accepted_key: accepted_key.clone(),
            };
            let config = Arc::clone(&config);
            tokio::spawn(async move {
                if let Ok(session) = server::run_stream(config, stream, handler).await {
                    let _ = session.await;
                }
            });
        }
    });

    Endpoint {
        host: "127.0.0.1".to_string(),
        port,
    }
}

fn password(text: &str) -> Auth {
    Auth::Password {
        username: USER.to_string(),
        password: Secret::new(text.to_string()),
    }
}

fn empty_known_hosts(directory: &tempfile::TempDir) -> KnownHosts {
    KnownHosts::load(&directory.path().join("known_hosts.toml")).unwrap()
}

/// Known hosts holding the test server's key, as after the user accepted it.
async fn trusting(endpoint: &Endpoint, directory: &tempfile::TempDir) -> KnownHosts {
    let mut known = empty_known_hosts(directory);
    let first = open_shell(endpoint, &password(PASSWORD), known.clone(), SIZE).await;
    let Err(Error::UnknownHostKey(offered)) = first else {
        panic!("the first contact should stop at the unknown host key");
    };
    known
        .trust(&endpoint.host, endpoint.port, &offered.key)
        .unwrap();
    known
}

/// Collects output until it contains `wanted`, failing after two seconds.
async fn read_until(shell: &mut Shell, wanted: &str) -> String {
    let mut seen = String::new();
    let reading = async {
        while !seen.contains(wanted) {
            match shell.events.recv().await {
                Some(ShellEvent::Output(bytes)) => seen.push_str(&String::from_utf8_lossy(&bytes)),
                other => panic!("expected output containing {wanted:?}, got {other:?}"),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(2), reading)
        .await
        .unwrap_or_else(|_| panic!("no output containing {wanted:?}"));
    seen
}

#[tokio::test]
async fn first_contact_stops_at_the_host_key_and_shows_its_fingerprint() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();

    let result = open_shell(
        &endpoint,
        &password(PASSWORD),
        empty_known_hosts(&directory),
        SIZE,
    )
    .await;

    match result {
        Err(Error::UnknownHostKey(key)) => {
            assert!(key.key.starts_with("ssh-ed25519 "));
            assert!(key.fingerprint.starts_with("SHA256:"));
        }
        _ => panic!("expected an unknown host key"),
    }
}

#[tokio::test]
async fn an_accepted_host_gives_a_shell_that_carries_bytes_both_ways() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;

    let mut shell = open_shell(&endpoint, &password(PASSWORD), known, SIZE)
        .await
        .unwrap();

    read_until(&mut shell, "pty xterm-256color 80x24").await;
    let typed = ShellInput::Data(b"hello\n".to_vec());
    shell.input.send(typed).await.unwrap();
    read_until(&mut shell, "echo hello").await;
}

#[tokio::test]
async fn a_resize_reaches_the_server() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    let mut shell = open_shell(&endpoint, &password(PASSWORD), known, SIZE)
        .await
        .unwrap();

    let resize = ShellInput::Resize(TerminalSize {
        columns: 132,
        lines: 43,
    });
    shell.input.send(resize).await.unwrap();

    read_until(&mut shell, "resized 132x43").await;
}

#[tokio::test]
async fn the_end_of_the_shell_is_reported_with_its_exit_status() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    let mut shell = open_shell(&endpoint, &password(PASSWORD), known, SIZE)
        .await
        .unwrap();

    let typed = ShellInput::Data(b"exit\n".to_vec());
    shell.input.send(typed).await.unwrap();

    let waiting = async {
        loop {
            match shell.events.recv().await {
                Some(ShellEvent::Output(_)) => {}
                Some(ShellEvent::Closed(status)) => return status,
                None => panic!("the events ended without a close"),
            }
        }
    };
    let status = tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .unwrap();
    assert_eq!(status, Some(7));
}

#[tokio::test]
async fn a_wrong_password_is_refused() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;

    let result = open_shell(&endpoint, &password("wrong"), known, SIZE).await;

    assert!(matches!(result, Err(Error::AuthRefused)));
}

#[tokio::test]
async fn a_changed_host_key_is_refused_before_signing_in() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let mut known = empty_known_hosts(&directory);
    let other_key = std::fs::read_to_string(fixture("client_key.pub")).unwrap();
    let other_key = other_key.rsplit_once(' ').unwrap().0;
    known
        .trust(&endpoint.host, endpoint.port, other_key)
        .unwrap();

    let result = open_shell(&endpoint, &password(PASSWORD), known, SIZE).await;

    assert!(matches!(result, Err(Error::HostKeyChanged(_))));
}

#[tokio::test]
async fn a_key_file_signs_in() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    let auth = Auth::KeyFile {
        username: USER.to_string(),
        path: fixture("client_key"),
        passphrase: None,
    };

    let mut shell = open_shell(&endpoint, &auth, known, SIZE).await.unwrap();

    read_until(&mut shell, "pty").await;
}

#[tokio::test]
async fn a_key_the_server_does_not_know_is_refused() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    let auth = Auth::KeyFile {
        username: USER.to_string(),
        path: fixture("locked_key"),
        passphrase: Some(Secret::new("open sesame".to_string())),
    };

    let result = open_shell(&endpoint, &auth, known, SIZE).await;

    assert!(matches!(result, Err(Error::AuthRefused)));
}

#[tokio::test]
async fn an_encrypted_key_without_its_passphrase_asks_for_it() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    let auth = Auth::KeyFile {
        username: USER.to_string(),
        path: fixture("locked_key"),
        passphrase: None,
    };

    let result = open_shell(&endpoint, &auth, known, SIZE).await;

    assert!(matches!(result, Err(Error::KeyLocked(_))));
}

#[tokio::test]
async fn a_closed_port_is_reported_as_unreachable() {
    // Bind and drop to get a port nothing listens on.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let endpoint = Endpoint {
        host: "127.0.0.1".to_string(),
        port,
    };
    let directory = tempfile::tempdir().unwrap();

    let result = open_shell(
        &endpoint,
        &password(PASSWORD),
        empty_known_hosts(&directory),
        SIZE,
    )
    .await;

    assert!(matches!(result, Err(Error::Unreachable { .. })));
}
