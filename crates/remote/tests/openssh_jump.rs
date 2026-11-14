//! A jump through a real OpenSSH server. Ignored by default: it needs one.
//!
//! The server connects back to itself: the jump goes to `RT_SSH_PORT`, and the
//! target is the same server as that server sees itself (`127.0.0.1:2222`).
//!
//! ```sh
//! RT_SSH_HOST=127.0.0.1 RT_SSH_PORT=2223 RT_SSH_USER=spike RT_SSH_KEY=/path/to/key \
//!     cargo nextest run -p remotething-remote --run-ignored all openssh_jump
//! ```

use std::path::PathBuf;
use std::time::Duration;

use remotething_remote::ssh::{
    Auth, Endpoint, Error, Hop, ShellEvent, ShellInput, TerminalSize, open_shell_through,
};
use remotething_secrets::KnownHosts;

fn setting(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this test"))
}

fn key_auth() -> Auth {
    Auth::KeyFile {
        username: setting("RT_SSH_USER"),
        path: PathBuf::from(setting("RT_SSH_KEY")),
        passphrase: None,
    }
}

#[tokio::test]
#[ignore = "needs an SSH server; see the top of this file"]
async fn a_real_openssh_server_relays_to_the_target() {
    let jump = Endpoint {
        host: setting("RT_SSH_HOST"),
        port: setting("RT_SSH_PORT").parse().unwrap(),
    };
    // As seen from the jump host itself.
    let target = Endpoint {
        host: "127.0.0.1".to_string(),
        port: 2222,
    };
    let size = TerminalSize {
        columns: 80,
        lines: 24,
    };
    let directory = tempfile::tempdir().unwrap();
    let mut known = KnownHosts::load(&directory.path().join("known_hosts.toml")).unwrap();

    // Trust each key as the app does when the user agrees, until the chain opens.
    let mut shell = loop {
        let jumps = [Hop {
            endpoint: jump.clone(),
            auth: key_auth(),
        }];
        match open_shell_through(&jumps, &target, &key_auth(), known.clone(), size).await {
            Ok(shell) => break shell,
            Err(Error::UnknownHostKey(key)) => {
                known
                    .trust(&key.endpoint.host, key.endpoint.port, &key.key)
                    .unwrap();
            }
            Err(other) => panic!("the jump failed: {other}"),
        }
    };

    shell
        .input
        .send(ShellInput::Data(
            b"echo through-the-jump-$((6*7))\n".to_vec(),
        ))
        .await
        .unwrap();
    let mut seen = String::new();
    let reading = async {
        while !seen.contains("through-the-jump-42") {
            if let Some(ShellEvent::Output(bytes)) = shell.events.recv().await {
                seen.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(5), reading)
        .await
        .unwrap_or_else(|_| panic!("no answer through the jump; saw {seen:?}"));
}
