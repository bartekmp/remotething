//! `open_shell` against a small SSH server running inside the test.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use remotething_remote::ssh::{
    Auth, Challenge, Endpoint, Error, Hop, Shell, ShellEvent, ShellInput, TerminalSize, open_shell,
    open_shell_running, open_shell_through, open_tunnel,
};
use remotething_secrets::{KnownHosts, Secret};
use russh::keys::PublicKey;
use russh::server::{self, Msg, Session};
use russh::{Channel, ChannelId};
use tokio::net::{TcpListener, TcpStream};

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
    /// How many connections this server relayed as a jump host.
    relayed: Arc<AtomicUsize>,
    /// Whether this server refuses to relay connections, like `AllowTcpForwarding no`.
    refuse_forwarding: bool,
    /// The channels running a shell. Relayed channels must not be echoed into.
    shells: Vec<ChannelId>,
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

    async fn auth_keyboard_interactive<'a>(
        &'a mut self,
        user: &str,
        _submethods: &str,
        response: Option<server::Response<'a>>,
    ) -> Result<server::Auth, Self::Error> {
        if user != USER {
            return Ok(server::Auth::reject());
        }
        let Some(response) = response else {
            // Like PAM with a one-time code: the password, then the code.
            return Ok(server::Auth::Partial {
                name: "".into(),
                instructions: "".into(),
                prompts: vec![
                    ("Password: ".into(), false),
                    ("Verification code: ".into(), false),
                ]
                .into(),
            });
        };
        let answers: Vec<Vec<u8>> = response.map(|answer| answer.to_vec()).collect();
        let right = answers == [PASSWORD.as_bytes(), b"123456"];
        Ok(if right {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
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

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _originator: &str,
        _originator_port: u32,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.refuse_forwarding {
            let refusal = russh::ChannelOpenFailure::AdministrativelyProhibited;
            reply.reject(refusal).await;
            return Ok(());
        }
        let Ok(mut target) = TcpStream::connect((host, port as u16)).await else {
            reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
            return Ok(());
        };
        self.relayed.fetch_add(1, Ordering::SeqCst);
        reply.accept().await;
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut target).await;
        });
        Ok(())
    }

    /// Listens for a remote forward and hands each client to the SSH client, like OpenSSH.
    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let Ok(listener) = TcpListener::bind((address, *port as u16)).await else {
            return Ok(false);
        };
        *port = u32::from(listener.local_addr().unwrap().port());
        let (address, port, handle) = (address.to_string(), *port, session.handle());
        tokio::spawn(async move {
            while let Ok((mut client, peer)) = listener.accept().await {
                let opened = handle
                    .channel_open_forwarded_tcpip(
                        address.clone(),
                        port,
                        peer.ip().to_string(),
                        u32::from(peer.port()),
                    )
                    .await;
                let Ok(channel) = opened else { continue };
                tokio::spawn(async move {
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut channel.into_stream())
                        .await;
                });
            }
        });
        Ok(true)
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
        self.shells.push(channel);
        session.channel_success(channel)?;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        command: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        let text = String::from_utf8_lossy(command);
        session.data(channel, format!("ran {text}\n"))?;
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
        if !self.shells.contains(&channel) {
            return Ok(());
        }
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
    start_counting_server().await.0
}

/// Like [`start_server`], also returning how many connections it has relayed.
async fn start_counting_server() -> (Endpoint, Arc<AtomicUsize>) {
    start_configured_server(false).await
}

/// Starts a server; with `refuse_forwarding` it will not relay connections.
async fn start_configured_server(refuse_forwarding: bool) -> (Endpoint, Arc<AtomicUsize>) {
    let host_key = russh::keys::load_secret_key(fixture("host_key"), None).unwrap();
    let accepted_key = russh::keys::load_public_key(fixture("client_key.pub")).unwrap();
    let config = Arc::new(server::Config {
        keys: vec![host_key],
        auth_rejection_time: Duration::ZERO,
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..server::Config::default()
    });

    let relayed = Arc::new(AtomicUsize::new(0));
    let relayed_by_server = Arc::clone(&relayed);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let handler = TestServer {
                accepted_key: accepted_key.clone(),
                relayed: Arc::clone(&relayed_by_server),
                refuse_forwarding,
                shells: Vec::new(),
            };
            let config = Arc::clone(&config);
            tokio::spawn(async move {
                if let Ok(session) = server::run_stream(config, stream, handler).await {
                    let _ = session.await;
                }
            });
        }
    });

    let endpoint = Endpoint {
        host: "127.0.0.1".to_string(),
        port,
    };
    (endpoint, relayed)
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
async fn a_command_runs_in_the_terminal_instead_of_the_login_shell() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;

    let mut shell = open_shell_running(
        &[],
        &endpoint,
        &password(PASSWORD),
        known,
        SIZE,
        Some("tmux new -A -s main"),
    )
    .await
    .unwrap();

    read_until(&mut shell, "pty xterm-256color 80x24").await;
    read_until(&mut shell, "ran tmux new -A -s main").await;
}

#[tokio::test]
async fn an_open_connection_tells_where_it_goes_and_who_answered() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;

    let shell = open_shell(&endpoint, &password(PASSWORD), known, SIZE)
        .await
        .unwrap();

    let details = shell.connection.details();
    assert_eq!(details.peer.map(|peer| peer.port()), Some(endpoint.port));
    assert!(details.server.starts_with("SSH-2.0-"), "{}", details.server);
    assert!(details.fingerprint.starts_with("SHA256:"));
    assert!(!details.host_key_algorithm.is_empty());
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

fn hop(endpoint: &Endpoint, password_text: &str) -> Hop {
    Hop {
        endpoint: endpoint.clone(),
        auth: password(password_text),
    }
}

#[tokio::test]
async fn a_shell_is_reached_through_a_jump_host() {
    let (jump, relayed) = start_counting_server().await;
    let target = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let mut known = trusting(&jump, &directory).await;
    // The same test key serves both servers, so it is trusted for the second address too.
    let Err(Error::UnknownHostKey(key)) =
        open_shell(&target, &password(PASSWORD), known.clone(), SIZE).await
    else {
        panic!("the target's key should still be unknown");
    };
    known.trust(&target.host, target.port, &key.key).unwrap();

    let jumps = [hop(&jump, PASSWORD)];
    let mut shell = open_shell_through(&jumps, &target, &password(PASSWORD), known, SIZE)
        .await
        .unwrap();

    read_until(&mut shell, "pty xterm-256color 80x24").await;
    shell
        .input
        .send(ShellInput::Data(b"over the jump\n".to_vec()))
        .await
        .unwrap();
    read_until(&mut shell, "echo over the jump").await;
    assert_eq!(relayed.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_unknown_jump_host_key_stops_there_and_names_the_jump_host() {
    let jump = start_server().await;
    let target = start_server().await;
    let directory = tempfile::tempdir().unwrap();

    let jumps = [hop(&jump, PASSWORD)];
    let result = open_shell_through(
        &jumps,
        &target,
        &password(PASSWORD),
        empty_known_hosts(&directory),
        SIZE,
    )
    .await;

    match result {
        Err(Error::UnknownHostKey(key)) => assert_eq!(key.endpoint, jump),
        _ => panic!("expected the jump host's key to be asked about"),
    }
}

#[tokio::test]
async fn a_refused_sign_in_on_the_jump_host_says_which_host() {
    let jump = start_server().await;
    let target = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&jump, &directory).await;

    let jumps = [hop(&jump, "wrong")];
    let result = open_shell_through(&jumps, &target, &password(PASSWORD), known, SIZE).await;

    match result {
        Err(Error::Jump { host, reason }) => {
            assert_eq!(host, jump.host);
            assert!(matches!(*reason, Error::AuthRefused));
        }
        _ => panic!("expected the jump host to be named"),
    }
}

#[tokio::test]
async fn a_target_the_jump_host_cannot_reach_is_unreachable() {
    let jump = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&jump, &directory).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed = Endpoint {
        host: "127.0.0.1".to_string(),
        port: listener.local_addr().unwrap().port(),
    };
    drop(listener);

    let jumps = [hop(&jump, PASSWORD)];
    let result = open_shell_through(&jumps, &closed, &password(PASSWORD), known, SIZE).await;

    assert!(matches!(result, Err(Error::Unreachable { .. })));
}

#[tokio::test]
async fn a_jump_host_that_forbids_forwarding_is_named_as_the_one_that_refused() {
    let (jump, _) = start_configured_server(true).await;
    let target = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&jump, &directory).await;

    let jumps = [hop(&jump, PASSWORD)];
    let result = open_shell_through(&jumps, &target, &password(PASSWORD), known, SIZE).await;

    match result {
        Err(Error::ForwardingRefused { host }) => assert_eq!(host, jump.host),
        _ => panic!("expected the jump host to be named as refusing"),
    }
}

/// A running ssh-agent that is stopped when dropped.
struct TestAgent {
    process: std::process::Child,
    socket: PathBuf,
    _directory: tempfile::TempDir,
}

impl TestAgent {
    /// Starts an agent and adds the keys named in `keys`.
    fn start(keys: &[&str]) -> TestAgent {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("agent.sock");
        let process = std::process::Command::new("ssh-agent")
            .args(["-D", "-a"])
            .arg(&socket)
            .spawn()
            .expect("ssh-agent must be installed to run this test");
        // The socket appears a moment after the process starts.
        for _ in 0..100 {
            if socket.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        for key in keys {
            let path = directory.path().join(key);
            std::fs::copy(fixture(key), &path).unwrap();
            // ssh-add refuses key files that others can read.
            std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
                .unwrap();
            let added = std::process::Command::new("ssh-add")
                .arg(&path)
                .env("SSH_AUTH_SOCK", &socket)
                .output()
                .expect("ssh-add must be installed to run this test");
            assert!(added.status.success(), "ssh-add failed");
        }
        TestAgent {
            process,
            socket,
            _directory: directory,
        }
    }

    fn auth(&self) -> Auth {
        Auth::Agent {
            username: USER.to_string(),
            socket: Some(self.socket.clone()),
        }
    }
}

impl Drop for TestAgent {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_key_held_by_the_agent_signs_in() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    // The agent also holds a key the server does not know; it must be skipped.
    let agent = TestAgent::start(&["host_key", "client_key"]);

    let mut shell = open_shell(&endpoint, &agent.auth(), known, SIZE)
        .await
        .unwrap();

    read_until(&mut shell, "pty").await;
}

#[cfg(unix)]
#[tokio::test]
async fn an_agent_whose_keys_the_server_does_not_accept_is_refused() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    let agent = TestAgent::start(&["host_key"]);

    let result = open_shell(&endpoint, &agent.auth(), known, SIZE).await;

    assert!(matches!(result, Err(Error::AuthRefused)));
}

#[cfg(unix)]
#[tokio::test]
async fn an_empty_agent_says_it_has_no_keys() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    let agent = TestAgent::start(&[]);

    let result = open_shell(&endpoint, &agent.auth(), known, SIZE).await;

    assert!(matches!(result, Err(Error::AgentEmpty)));
}

#[cfg(unix)]
#[tokio::test]
async fn a_missing_agent_says_none_is_running() {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    let auth = Auth::Agent {
        username: USER.to_string(),
        socket: Some(directory.path().join("no-agent.sock")),
    };

    let result = open_shell(&endpoint, &auth, known, SIZE).await;

    assert!(matches!(result, Err(Error::AgentUnavailable(_))));
}

/// A TCP server that answers every line with the line in capitals.
async fn start_shouting_server() -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buffer = [0u8; 64];
                while let Ok(read) = stream.read(&mut buffer).await {
                    if read == 0 {
                        return;
                    }
                    let shouted = buffer[..read].to_ascii_uppercase();
                    let _ = stream.write_all(&shouted).await;
                }
            });
        }
    });
    port
}

async fn connected_shell() -> Shell {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    open_shell(&endpoint, &password(PASSWORD), known, SIZE)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_local_forward_reaches_a_host_behind_the_server() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let target = start_shouting_server().await;
    let shell = connected_shell().await;
    let forward = shell
        .connection
        .forward_local(0, "127.0.0.1".to_string(), target)
        .await
        .unwrap();

    let mut client = tokio::net::TcpStream::connect(("127.0.0.1", forward.local_port()))
        .await
        .unwrap();
    client.write_all(b"hello").await.unwrap();
    let mut answer = [0u8; 5];
    client.read_exact(&mut answer).await.unwrap();

    assert_eq!(&answer, b"HELLO");
}

#[tokio::test]
async fn a_remote_forward_lets_the_server_reach_a_host_here() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let target = start_shouting_server().await;
    let shell = connected_shell().await;
    let forward = shell
        .connection
        .forward_remote(0, "127.0.0.1".to_string(), target)
        .await
        .unwrap();

    let mut client = tokio::net::TcpStream::connect(("127.0.0.1", forward.local_port()))
        .await
        .unwrap();
    client.write_all(b"hello").await.unwrap();
    let mut answer = [0u8; 5];
    client.read_exact(&mut answer).await.unwrap();

    assert_eq!(&answer, b"HELLO");
}

#[tokio::test]
async fn a_dynamic_forward_is_a_socks_proxy_through_the_server() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let target = start_shouting_server().await;
    let shell = connected_shell().await;
    let forward = shell.connection.forward_dynamic(0).await.unwrap();

    let mut client = tokio::net::TcpStream::connect(("127.0.0.1", forward.local_port()))
        .await
        .unwrap();
    // Greeting with one method (no sign-in), then CONNECT to 127.0.0.1:target.
    client.write_all(&[5, 1, 0]).await.unwrap();
    let mut chosen = [0u8; 2];
    client.read_exact(&mut chosen).await.unwrap();
    assert_eq!(chosen, [5, 0]);
    let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
    request.extend_from_slice(&target.to_be_bytes());
    client.write_all(&request).await.unwrap();
    let mut reply = [0u8; 10];
    client.read_exact(&mut reply).await.unwrap();
    assert_eq!(&reply[..2], &[5, 0]);
    client.write_all(b"hello").await.unwrap();
    let mut answer = [0u8; 5];
    client.read_exact(&mut answer).await.unwrap();

    assert_eq!(&answer, b"HELLO");
}

#[tokio::test]
async fn a_dynamic_forward_answers_refused_for_a_closed_port() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let shell = connected_shell().await;
    let forward = shell.connection.forward_dynamic(0).await.unwrap();

    let mut client = tokio::net::TcpStream::connect(("127.0.0.1", forward.local_port()))
        .await
        .unwrap();
    client.write_all(&[5, 1, 0]).await.unwrap();
    let mut chosen = [0u8; 2];
    client.read_exact(&mut chosen).await.unwrap();
    client
        .write_all(&[5, 1, 0, 1, 127, 0, 0, 1, 0, 9])
        .await
        .unwrap();
    let mut reply = [0u8; 10];
    client.read_exact(&mut reply).await.unwrap();

    assert_eq!(reply[1], 5);
}

#[tokio::test]
async fn a_tunnel_reaches_a_host_behind_the_server() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let behind = start_shouting_server().await;
    let server = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&server, &directory).await;
    let target = Endpoint {
        host: "127.0.0.1".to_string(),
        port: behind,
    };

    let mut tunnel = open_tunnel(&[hop(&server, PASSWORD)], &target, known)
        .await
        .unwrap();
    tunnel.write_all(b"hello").await.unwrap();
    let mut answer = [0u8; 5];
    tunnel.read_exact(&mut answer).await.unwrap();

    assert_eq!(&answer, b"HELLO");
}

#[tokio::test]
async fn a_tunnel_through_a_server_that_forbids_forwarding_names_it() {
    let (server, _) = start_configured_server(true).await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&server, &directory).await;
    let target = Endpoint {
        host: "127.0.0.1".to_string(),
        port: 9,
    };

    let result = open_tunnel(&[hop(&server, PASSWORD)], &target, known).await;

    match result {
        Err(Error::ForwardingRefused { host }) => assert_eq!(host, server.host),
        _ => panic!("expected the server to be named as refusing"),
    }
}

#[tokio::test]
async fn a_tunnel_needs_a_server_to_go_through() {
    let directory = tempfile::tempdir().unwrap();
    let known = empty_known_hosts(&directory);
    let target = Endpoint {
        host: "127.0.0.1".to_string(),
        port: 9,
    };

    let result = open_tunnel(&[], &target, known).await;

    assert!(matches!(result, Err(Error::Protocol(_))));
}

#[tokio::test]
async fn a_port_that_is_taken_is_reported() {
    let taken = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = taken.local_addr().unwrap().port();
    let shell = connected_shell().await;

    let result = shell
        .connection
        .forward_local(port, "127.0.0.1".to_string(), 1)
        .await;

    assert!(matches!(result, Err(Error::ListenFailed { port: p, .. }) if p == port));
}

#[tokio::test]
async fn dropping_a_forward_closes_its_port() {
    let shell = connected_shell().await;
    let forward = shell
        .connection
        .forward_local(0, "127.0.0.1".to_string(), 1)
        .await
        .unwrap();
    let port = forward.local_port();

    drop(forward);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err()
    );
}

/// Signs in interactively, answering each question with `answer`; `None` cancels.
async fn interactive_attempt(
    answer: impl Fn(&Challenge) -> Option<Vec<Secret>> + Send + 'static,
) -> (Result<Shell, Error>, Vec<String>) {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    let (challenges, mut asked) = tokio::sync::mpsc::channel::<Challenge>(4);
    let auth = Auth::Interactive {
        username: USER.to_string(),
        challenges,
    };
    let user = tokio::spawn(async move {
        let mut seen = Vec::new();
        while let Some(challenge) = asked.recv().await {
            seen.extend(challenge.prompts.iter().map(|prompt| prompt.text.clone()));
            if let Some(answers) = answer(&challenge) {
                let _ = challenge.reply.send(answers);
            }
        }
        seen
    });
    let result = open_shell(&endpoint, &auth, known, SIZE).await;
    drop(auth);
    (result, user.await.unwrap())
}

fn secrets(texts: &[&str]) -> Vec<Secret> {
    texts
        .iter()
        .map(|text| Secret::new(text.to_string()))
        .collect()
}

#[tokio::test]
async fn keyboard_interactive_asks_the_servers_questions_and_signs_in() {
    let (result, asked) = interactive_attempt(|_| Some(secrets(&[PASSWORD, "123456"]))).await;

    assert!(result.is_ok());
    assert_eq!(asked, ["Password: ", "Verification code: "]);
}

#[tokio::test]
async fn a_wrong_one_time_code_is_refused() {
    let (result, _) = interactive_attempt(|_| Some(secrets(&[PASSWORD, "000000"]))).await;

    assert!(matches!(result, Err(Error::AuthRefused)));
}

#[tokio::test]
async fn dismissing_the_questions_cancels_the_sign_in() {
    let (result, _) = interactive_attempt(|_| None).await;

    assert!(matches!(result, Err(Error::SignInCancelled)));
}

async fn try_key_file(name: &str, passphrase: Option<&str>) -> Result<Shell, Error> {
    let endpoint = start_server().await;
    let directory = tempfile::tempdir().unwrap();
    let known = trusting(&endpoint, &directory).await;
    let auth = Auth::KeyFile {
        username: USER.to_string(),
        path: fixture(name),
        passphrase: passphrase.map(|text| Secret::new(text.to_string())),
    };
    open_shell(&endpoint, &auth, known, SIZE).await
}

#[tokio::test]
async fn a_putty_key_file_signs_in() {
    let result = try_key_file("client_key.ppk", None).await;
    assert!(result.is_ok(), "{:?}", result.err());
}

#[tokio::test]
async fn an_encrypted_putty_key_asks_for_its_passphrase() {
    let result = try_key_file("locked_key.ppk", None).await;

    assert!(
        matches!(result, Err(Error::KeyLocked(_))),
        "{:?}",
        result.err()
    );
}

#[tokio::test]
async fn an_encrypted_putty_key_signs_in_with_its_passphrase() {
    let result = try_key_file("locked_key.ppk", Some("open sesame")).await;
    assert!(result.is_ok(), "{:?}", result.err());
}

#[tokio::test]
async fn a_wrong_putty_passphrase_is_not_a_request_for_one() {
    let result = try_key_file("locked_key.ppk", Some("wrong")).await;

    assert!(
        matches!(result, Err(Error::KeyFile { .. })),
        "{:?}",
        result.err()
    );
}
