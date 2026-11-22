//! Host certificates against a real OpenSSH server, started here on a free port.
//! Skipped when `sshd` or `ssh-keygen` is not installed.
//!
//! The authorities are given as the OpenSSH `known_hosts` file the client reads.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use remotething_remote::ssh::{Auth, Endpoint, Error, TerminalSize, open_shell};
use remotething_secrets::KnownHosts;

const SSHD: &str = "/usr/sbin/sshd";

fn keygen(directory: &Path, arguments: &[&str]) {
    let status = Command::new("ssh-keygen")
        .args(arguments)
        .current_dir(directory)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("ssh-keygen runs");
    assert!(status.success(), "ssh-keygen {arguments:?} failed");
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct Server {
    child: Child,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A server whose host key has a certificate from the authority `ca`.
fn serve(directory: &Path, port: u16, ca: &str) -> Server {
    keygen(directory, &["-q", "-t", "ed25519", "-N", "", "-f", "host"]);
    keygen(
        directory,
        &[
            "-q",
            "-s",
            ca,
            "-h",
            "-I",
            "test",
            "-n",
            "127.0.0.1",
            "host.pub",
        ],
    );
    let config = format!(
        "Port {port}\nListenAddress 127.0.0.1\nHostKey {d}/host\nHostCertificate {d}/host-cert.pub\n\
         AuthorizedKeysFile {d}/user.pub\nPidFile {d}/sshd.pid\nPasswordAuthentication no\n\
         UsePAM no\nStrictModes no\n",
        d = directory.display()
    );
    std::fs::write(directory.join("sshd_config"), config).unwrap();
    let child = Command::new(SSHD)
        .args(["-D", "-e", "-f"])
        .arg(directory.join("sshd_config"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("sshd starts");
    Server { child }
}

async fn connect(port: u16, key: PathBuf, openssh_file: &Path) -> Result<(), Error> {
    let endpoint = Endpoint {
        host: "127.0.0.1".to_string(),
        port,
    };
    let auth = Auth::KeyFile {
        username: std::env::var("USER").unwrap_or_else(|_| "root".to_string()),
        path: key,
        passphrase: None,
    };
    let known = KnownHosts::default().with_openssh_file(openssh_file.to_path_buf());
    let size = TerminalSize {
        columns: 80,
        lines: 24,
    };
    // The server needs a moment to listen.
    for _ in 0..50 {
        match open_shell(&endpoint, &auth, known.clone(), size).await {
            Err(Error::Unreachable { .. }) => tokio::time::sleep(Duration::from_millis(100)).await,
            other => return other.map(|_| ()),
        }
    }
    panic!("the server never listened");
}

fn trust_authority(file: &Path, patterns: &str, ca_public: &Path) {
    let key = std::fs::read_to_string(ca_public).unwrap();
    let line = format!("@cert-authority {patterns} {}\n", key.trim());
    std::fs::write(file, line).unwrap();
}

#[tokio::test]
async fn a_server_with_a_certificate_is_trusted_through_its_authority() {
    if !Path::new(SSHD).exists() || Command::new("ssh-keygen").arg("-?").output().is_err() {
        eprintln!("skipped: sshd or ssh-keygen is not installed");
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let dir = directory.path();
    keygen(dir, &["-q", "-t", "ed25519", "-N", "", "-f", "ca"]);
    keygen(dir, &["-q", "-t", "ed25519", "-N", "", "-f", "other-ca"]);
    keygen(dir, &["-q", "-t", "ed25519", "-N", "", "-f", "user"]);
    let file = dir.join("openssh_known_hosts");

    let port = free_port();
    let _server = serve(dir, port, "ca");
    let key = dir.join("user");
    // OpenSSH writes a host on another port than 22 like this.
    let host = format!("[127.0.0.1]:{port}");

    // No authority: the certificate is not asked for, so the plain key is unknown.
    let result = connect(port, key.clone(), &file).await;
    assert!(
        matches!(result, Err(Error::UnknownHostKey(_))),
        "{result:?}"
    );

    // The right authority: accepted with nothing to confirm.
    trust_authority(&file, &host, &dir.join("ca.pub"));
    let result = connect(port, key.clone(), &file).await;
    assert!(result.is_ok(), "{result:?}");

    // The authority for other hosts only: not asked for, so the plain key is unknown again.
    trust_authority(&file, "*.example.net", &dir.join("ca.pub"));
    let result = connect(port, key.clone(), &file).await;
    assert!(
        matches!(result, Err(Error::UnknownHostKey(_))),
        "{result:?}"
    );

    // An authority that did not sign the certificate: refused outright.
    trust_authority(&file, &host, &dir.join("other-ca.pub"));
    let result = connect(port, key, &file).await;
    assert!(matches!(result, Err(Error::Protocol(_))), "{result:?}");
}
