//! SCP against a real OpenSSH server that has no SFTP. Ignored by default: it needs a server.
//!
//! ```sh
//! RT_SCP_HOST=127.0.0.1 RT_SCP_PORT=2298 RT_SCP_USER=me RT_SCP_KEY=/path/to/key \
//!     cargo nextest run -p remotething-files --run-ignored all scp
//! ```

use std::path::PathBuf;

use remotething_files::{EntryKind, RemoteFs, ScpFs, SftpFs, join};
use remotething_remote::ssh::{Auth, Connection, Endpoint, Error, open_connection_through};
use remotething_secrets::KnownHosts;

fn setting(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this test"))
}

async fn connection(directory: &std::path::Path) -> Connection {
    let endpoint = Endpoint {
        host: setting("RT_SCP_HOST"),
        port: setting("RT_SCP_PORT").parse().unwrap(),
    };
    let auth = Auth::KeyFile {
        username: setting("RT_SCP_USER"),
        path: PathBuf::from(setting("RT_SCP_KEY")),
        passphrase: None,
    };
    let mut known = KnownHosts::load(&directory.join("known_hosts.toml")).unwrap();
    let Err(Error::UnknownHostKey(key)) =
        open_connection_through(&[], &endpoint, &auth, known.clone()).await
    else {
        panic!("the first contact should stop at the host key");
    };
    known
        .trust(&endpoint.host, endpoint.port, &key.key)
        .unwrap();
    open_connection_through(&[], &endpoint, &auth, known)
        .await
        .unwrap()
}

async fn connect(directory: &std::path::Path) -> ScpFs {
    ScpFs::open(&connection(directory).await).await.unwrap()
}

#[tokio::test]
#[ignore = "needs an SSH server; see the top of this file"]
async fn a_server_without_sftp_refuses_it_quickly_so_the_app_can_fall_back() {
    let directory = tempfile::tempdir().unwrap();
    let connection = connection(directory.path()).await;

    let attempt = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        SftpFs::open(&connection),
    )
    .await;

    assert!(matches!(attempt, Ok(Err(_))), "SFTP should be refused");
}

#[tokio::test]
#[ignore = "needs an SSH server; see the top of this file"]
async fn files_and_folders_go_up_are_listed_and_come_back_the_same() {
    let directory = tempfile::tempdir().unwrap();
    let files = connect(directory.path()).await;
    let home = files.home().await.unwrap();
    assert!(home.starts_with('/'));
    let top = join(&home, "remotething scp test");
    let _ = files.remove_file(&join(&top, "a/it's.bin")).await;

    // A file with awkward characters in nested folders that do not exist yet.
    let content: Vec<u8> = (0..300_000u32).map(|n| (n % 251) as u8).collect();
    let local = directory.path().join("up.bin");
    std::fs::write(&local, &content).unwrap();
    let remote = join(&top, "a/it's.bin");
    let mut last = 0;
    let sent = files
        .upload_tracked(&local, &remote, false, &mut |p| last = p.done)
        .await
        .unwrap();
    assert_eq!((sent.bytes, last), (300_000, 300_000));
    assert!(files.exists(&remote).await.unwrap());
    assert!(!files.exists(&join(&top, "nothing")).await.unwrap());
    assert_eq!(files.size(&remote).await.unwrap(), 300_000);

    let listing = files.list(&join(&top, "a")).await.unwrap();
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].name, "it's.bin");
    assert_eq!(listing[0].kind, EntryKind::File);
    assert_eq!(listing[0].size, Some(300_000));
    assert_eq!(
        files.files_under(&top).await.unwrap(),
        [("a/it's.bin".to_string(), 300_000)]
    );

    let back = directory.path().join("x/down.bin");
    let received = files
        .download_tracked(&remote, &back, false, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(received.bytes, 300_000);
    assert_eq!(std::fs::read(&back).unwrap(), content);

    // Preview, rename, and the errors.
    let (start, longer) = files.read_start(&remote, 4).await.unwrap();
    assert_eq!((start.as_slice(), longer), (&content[..4], true));
    let renamed = join(&top, "a/renamed.bin");
    files.rename(&remote, &renamed).await.unwrap();
    assert!(!files.exists(&remote).await.unwrap());
    assert!(files.rename(&renamed, &join(&top, "a")).await.is_err());
    assert!(files.list("/definitely/not/here").await.is_err());
    assert!(
        files
            .download_tracked(&remote, &back, false, &mut |_| {})
            .await
            .is_err()
    );

    // Clean up; a folder with something in it is refused.
    assert!(files.remove_folder(&join(&top, "a")).await.is_err());
    files.remove_file(&renamed).await.unwrap();
    files.remove_folder(&join(&top, "a")).await.unwrap();
    files.remove_folder(&top).await.unwrap();
}
