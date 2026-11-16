//! SFTP against a real OpenSSH server. Ignored by default: it needs a server.
//!
//! ```sh
//! RT_SFTP_HOST=127.0.0.1 RT_SFTP_PORT=2223 RT_SFTP_USER=spike RT_SFTP_KEY=/path/to/key \
//!     cargo nextest run -p remotething-files --run-ignored all
//! ```

use std::path::PathBuf;

use remotething_files::{EntryKind, RemoteFs, SftpFs, join};
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
    let sent = files
        .upload_tracked(&local, &remote, false, &mut |_| {})
        .await
        .unwrap()
        .bytes;
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
    let received = files
        .download_tracked(&remote, &back, false, &mut |_| {})
        .await
        .unwrap()
        .bytes;
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
        .upload_tracked(&part, &remote, false, &mut |_| {})
        .await
        .unwrap();
    let mut seen = Vec::new();
    let sent = files
        .upload_tracked(&whole, &remote, true, &mut |progress| seen.push(progress))
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
        .download_tracked(&remote, &back, true, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(received.resumed_from, 150_000);
    assert_eq!(std::fs::read(&back).unwrap(), content);

    // Without resuming, the file is replaced from the start.
    let again = files
        .download_tracked(&remote, &back, false, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(again.resumed_from, 0);
    assert_eq!(std::fs::read(&back).unwrap(), content);
}

#[tokio::test]
#[ignore = "needs an SSH server; see the top of this file"]
async fn a_folder_tree_goes_up_and_is_walked_and_comes_back() {
    let directory = tempfile::tempdir().unwrap();
    let files = connect(directory.path()).await;
    let home = files.home().await.unwrap();
    let top = join(&home, "remotething-test-tree");

    // Files in nested folders that do not exist yet: the folders are made on the way.
    let local = directory.path().join("one.txt");
    std::fs::write(&local, "one").unwrap();
    files
        .upload_tracked(&local, &join(&top, "a/b/one.txt"), false, &mut |_| {})
        .await
        .unwrap();
    files
        .upload_tracked(&local, &join(&top, "two.txt"), false, &mut |_| {})
        .await
        .unwrap();

    let found = files.files_under(&top).await.unwrap();
    assert_eq!(
        found,
        [("a/b/one.txt".to_string(), 3), ("two.txt".to_string(), 3)]
    );

    // A download into folders that do not exist yet.
    let target = directory.path().join("x/y/one.txt");
    files
        .download_tracked(&join(&top, "a/b/one.txt"), &target, false, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "one");
}

#[tokio::test]
#[ignore = "needs an SSH server; see the top of this file"]
async fn the_start_of_a_file_is_read_into_memory_and_a_longer_file_is_cut() {
    let directory = tempfile::tempdir().unwrap();
    let files = connect(directory.path()).await;
    let home = files.home().await.unwrap();
    let remote = join(&home, "remotething-test-preview.txt");
    let local = directory.path().join("p.txt");
    std::fs::write(&local, "0123456789").unwrap();
    files
        .upload_tracked(&local, &remote, false, &mut |_| {})
        .await
        .unwrap();

    let (start, longer) = files.read_start(&remote, 4).await.unwrap();
    assert_eq!((start.as_slice(), longer), (b"0123".as_slice(), true));

    let (all, longer) = files.read_start(&remote, 10).await.unwrap();
    assert_eq!((all.as_slice(), longer), (b"0123456789".as_slice(), false));

    let (all, longer) = files.read_start(&remote, 100).await.unwrap();
    assert_eq!((all.len(), longer), (10, false));
}

#[tokio::test]
#[ignore = "needs an SSH server; see the top of this file"]
async fn files_and_folders_are_made_renamed_and_deleted_and_nothing_is_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let files = connect(directory.path()).await;
    let home = files.home().await.unwrap();
    let top = join(&home, "remotething-test-ops");
    let local = directory.path().join("f.txt");
    std::fs::write(&local, "x").unwrap();

    files.create_folder(&top).await.unwrap();
    assert!(
        files.create_folder(&top).await.is_err(),
        "a taken path is refused"
    );

    let first = join(&top, "first.txt");
    let second = join(&top, "second.txt");
    files
        .upload_tracked(&local, &first, false, &mut |_| {})
        .await
        .unwrap();
    files
        .upload_tracked(&local, &second, false, &mut |_| {})
        .await
        .unwrap();
    assert!(
        files.rename(&first, &second).await.is_err(),
        "renaming onto an existing file must not replace it"
    );
    let renamed = join(&top, "renamed.txt");
    files.rename(&first, &renamed).await.unwrap();
    assert!(!files.exists(&first).await.unwrap());
    assert!(files.exists(&renamed).await.unwrap());

    assert!(
        files.remove_folder(&top).await.is_err(),
        "a folder with files stays"
    );
    files.remove_file(&renamed).await.unwrap();
    files.remove_file(&second).await.unwrap();
    files.remove_folder(&top).await.unwrap();
    assert!(!files.exists(&top).await.unwrap());
}
