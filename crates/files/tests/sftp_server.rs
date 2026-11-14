//! SFTP against a real OpenSSH server. Ignored by default: it needs a server.
//!
//! ```sh
//! RT_SFTP_HOST=127.0.0.1 RT_SFTP_PORT=2223 RT_SFTP_USER=spike RT_SFTP_KEY=/path/to/key \
//!     cargo nextest run -p remotething-files --run-ignored all
//! ```

use std::path::PathBuf;

use remotething_files::{EntryKind, SftpFs, join};
use remotething_remote::ssh::{Auth, Endpoint, Error, TerminalSize, open_shell};
use remotething_secrets::KnownHosts;

fn setting(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this test"))
}

#[tokio::test]
#[ignore = "needs an SSH server; see the top of this file"]
async fn a_file_goes_up_is_listed_and_comes_back_the_same() {
    let endpoint = Endpoint {
        host: setting("RT_SFTP_HOST"),
        port: setting("RT_SFTP_PORT").parse().unwrap(),
    };
    let auth = Auth::KeyFile {
        username: setting("RT_SFTP_USER"),
        path: PathBuf::from(setting("RT_SFTP_KEY")),
        passphrase: None,
    };
    let size = TerminalSize {
        columns: 80,
        lines: 24,
    };
    let directory = tempfile::tempdir().unwrap();

    // Trust the test server's key the way the app does after the user agrees.
    let mut known = KnownHosts::load(&directory.path().join("known_hosts.toml")).unwrap();
    let Err(Error::UnknownHostKey(key)) = open_shell(&endpoint, &auth, known.clone(), size).await
    else {
        panic!("the first contact should stop at the host key");
    };
    known
        .trust(&endpoint.host, endpoint.port, &key.key)
        .unwrap();
    let shell = open_shell(&endpoint, &auth, known, size).await.unwrap();

    let files = SftpFs::open(&shell.connection).await.unwrap();
    let home = files.home().await.unwrap();
    assert!(home.starts_with('/'));

    let local = directory.path().join("up.txt");
    let content = "remotething\n".repeat(20_000);
    std::fs::write(&local, &content).unwrap();
    let remote = join(&home, "remotething-test-upload.txt");

    let sent = files.upload(&local, &remote).await.unwrap();
    assert_eq!(sent, content.len() as u64);

    let listing = files.list(&home).await.unwrap();
    let entry = listing
        .iter()
        .find(|entry| entry.name == "remotething-test-upload.txt")
        .expect("the uploaded file is listed");
    assert_eq!(entry.kind, EntryKind::File);
    assert_eq!(entry.size, Some(content.len() as u64));
    assert!(entry.modified.is_some());
    assert!(
        listing
            .iter()
            .all(|entry| entry.name != "." && entry.name != "..")
    );

    let back = directory.path().join("down.txt");
    let received = files.download(&remote, &back).await.unwrap();
    assert_eq!(received, content.len() as u64);
    assert_eq!(std::fs::read_to_string(&back).unwrap(), content);

    assert!(files.list("/definitely/not/here").await.is_err());
}
