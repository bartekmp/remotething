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

/// Connects to the test server and starts SFTP, trusting its key like the app does after the user agrees.
async fn connect(directory: &std::path::Path) -> SftpFs {
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
    // Trust the test server's key the way the app does after the user agrees.
    let mut known = KnownHosts::load(&directory.join("known_hosts.toml")).unwrap();
    let Err(Error::UnknownHostKey(key)) = open_shell(&endpoint, &auth, known.clone(), size).await
    else {
        panic!("the first contact should stop at the host key");
    };
    known
        .trust(&endpoint.host, endpoint.port, &key.key)
        .unwrap();
    let shell = open_shell(&endpoint, &auth, known, size).await.unwrap();

    SftpFs::open(&shell.connection).await.unwrap()
}

#[tokio::test]
#[ignore = "needs an SSH server; see the top of this file"]
async fn a_file_goes_up_is_listed_and_comes_back_the_same() {
    let directory = tempfile::tempdir().unwrap();
    let files = connect(directory.path()).await;
    let home = files.home().await.unwrap();
    assert!(home.starts_with('/'));

    let local = directory.path().join("up.txt");
    let content = "remotething\n".repeat(20_000);
    std::fs::write(&local, &content).unwrap();
    let remote = join(&home, "remotething-test-upload.txt");

    assert!(!files.exists(&remote).await.unwrap());
    let sent = files.upload(&local, &remote).await.unwrap();
    assert!(files.exists(&remote).await.unwrap());
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

#[tokio::test]
#[ignore = "needs an SSH server; see the top of this file"]
async fn a_cut_short_transfer_carries_on_where_it_stopped() {
    let directory = tempfile::tempdir().unwrap();
    let files = connect(directory.path()).await;
    let home = files.home().await.unwrap();
    let content: Vec<u8> = (0..300_000u32).map(|n| (n % 251) as u8).collect();
    let whole = directory.path().join("whole.bin");
    std::fs::write(&whole, &content).unwrap();
    let remote = join(&home, "remotething-test-resume.bin");

    // An upload that stopped after 100 000 bytes.
    let part = directory.path().join("part.bin");
    std::fs::write(&part, &content[..100_000]).unwrap();
    files
        .upload_tracked(&part, &remote, false, |_| {})
        .await
        .unwrap();
    let mut seen = Vec::new();
    let sent = files
        .upload_tracked(&whole, &remote, true, |progress| seen.push(progress))
        .await
        .unwrap();
    assert_eq!(sent.resumed_from, 100_000);
    assert_eq!(sent.bytes, 300_000);
    assert_eq!(seen.first().map(|progress| progress.done), Some(100_000));
    assert_eq!(files.size(&remote).await.unwrap(), 300_000);

    // A download that stopped after 150 000 bytes.
    let back = directory.path().join("back.bin");
    std::fs::write(&back, &content[..150_000]).unwrap();
    let received = files
        .download_tracked(&remote, &back, true, |_| {})
        .await
        .unwrap();
    assert_eq!(received.resumed_from, 150_000);
    assert_eq!(std::fs::read(&back).unwrap(), content);

    // Without resuming, the file is replaced from the start.
    let again = files
        .download_tracked(&remote, &back, false, |_| {})
        .await
        .unwrap();
    assert_eq!(again.resumed_from, 0);
    assert_eq!(std::fs::read(&back).unwrap(), content);
}
