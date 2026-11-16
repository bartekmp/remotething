//! FTP and FTPS against a real server. Ignored by default: it needs one.
//!
//! ```sh
//! RT_FTP_HOST=127.0.0.1 RT_FTP_PORT=2121 RT_FTP_USER=test RT_FTP_PASSWORD=... \
//!     cargo test -p remotething-files --test ftp_server -- --ignored
//! ```
//!
//! Set `RT_FTPS_PORT` too to run the FTPS test against a server with TLS.

use remotething_files::{Certificate, EntryKind, Error, FtpFs, FtpSettings, RemoteFs, join};
use remotething_secrets::{KnownHosts, Secret};

fn setting(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this test"))
}

fn settings(port: &str, password: &str, secure: Option<KnownHosts>) -> FtpSettings {
    FtpSettings {
        host: setting("RT_FTP_HOST"),
        port: port.parse().unwrap(),
        username: setting("RT_FTP_USER"),
        password: Secret::new(password.to_string()),
        secure,
    }
}

/// Everything the file pane does, against whatever server `files` is on, in a folder of its own
/// so tests running side by side do not meet.
async fn exercise(files: &dyn RemoteFs, scratch: &std::path::Path, folder: &str) {
    let home = files.home().await.unwrap();
    let top = join(&home, folder);
    let _ = files.create_folders(&top).await;
    assert!(files.exists(&top).await.unwrap());

    // An upload into folders that do not exist yet, listed and read back.
    let content: Vec<u8> = (0..300_000u32).map(|n| (n % 251) as u8).collect();
    let whole = scratch.join("whole.bin");
    std::fs::write(&whole, &content).unwrap();
    let remote = join(&top, "a/b/data.bin");
    let sent = files
        .upload_tracked(&whole, &remote, false, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(sent.bytes, 300_000);
    assert_eq!(files.size(&remote).await.unwrap(), 300_000);
    let listing = files.list(&join(&top, "a/b")).await.unwrap();
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].name, "data.bin");
    assert_eq!(listing[0].kind, EntryKind::File);
    assert_eq!(listing[0].size, Some(300_000));
    let folders = files.list(&top).await.unwrap();
    assert!(
        folders
            .iter()
            .any(|e| e.name == "a" && e.kind == EntryKind::Folder)
    );

    // A cut-short upload carries on where it stopped.
    let part = scratch.join("part.bin");
    std::fs::write(&part, &content[..100_000]).unwrap();
    let resumable = join(&top, "resumable.bin");
    files
        .upload_tracked(&part, &resumable, false, &mut |_| {})
        .await
        .unwrap();
    let carried = files
        .upload_tracked(&whole, &resumable, true, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(carried.resumed_from, 100_000);
    assert_eq!(files.size(&resumable).await.unwrap(), 300_000);

    // A download, whole and resumed, comes back identical.
    let back = scratch.join("x/y/back.bin");
    files
        .download_tracked(&remote, &back, false, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(std::fs::read(&back).unwrap(), content);
    let cut = scratch.join("cut.bin");
    std::fs::write(&cut, &content[..150_000]).unwrap();
    let carried = files
        .download_tracked(&resumable, &cut, true, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(carried.resumed_from, 150_000);
    assert_eq!(std::fs::read(&cut).unwrap(), content);

    // The start of a file, and the walk of a folder.
    let (start, longer) = files.read_start(&remote, 4).await.unwrap();
    assert_eq!((start.as_slice(), longer), (&content[..4], true));
    let walked = files.files_under(&top).await.unwrap();
    assert_eq!(
        walked
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["a/b/data.bin", "resumable.bin"]
    );

    // Rename never replaces; a free name works; delete needs an empty folder.
    assert!(files.rename(&resumable, &remote).await.is_err());
    let renamed = join(&top, "renamed.bin");
    files.rename(&resumable, &renamed).await.unwrap();
    assert!(!files.exists(&resumable).await.unwrap());
    assert!(files.exists(&renamed).await.unwrap());
    assert!(files.remove_folder(&join(&top, "a")).await.is_err());
    files.remove_file(&renamed).await.unwrap();
    files.remove_file(&remote).await.unwrap();
    files.remove_folder(&join(&top, "a/b")).await.unwrap();
    files.remove_folder(&join(&top, "a")).await.unwrap();
    files.remove_folder(&top).await.unwrap();
    assert!(!files.exists(&top).await.unwrap());
}

#[tokio::test]
#[ignore = "needs an FTP server; see the top of this file"]
async fn plain_ftp_does_everything_the_file_pane_needs() {
    let files = FtpFs::connect(settings(
        &setting("RT_FTP_PORT"),
        &setting("RT_FTP_PASSWORD"),
        None,
    ))
    .await
    .unwrap();
    let scratch = tempfile::tempdir().unwrap();

    exercise(&files, scratch.path(), "remotething-test-plain").await;
}

#[tokio::test]
#[ignore = "needs an FTP server; see the top of this file"]
async fn a_wrong_password_is_refused_as_such() {
    let result = FtpFs::connect(settings(&setting("RT_FTP_PORT"), "not-the-password", None)).await;

    assert!(matches!(result, Err(Error::LoginRefused)));
}

#[tokio::test]
#[ignore = "needs an FTPS server; see the top of this file"]
async fn ftps_stops_at_an_unknown_certificate_and_works_once_it_is_trusted() {
    let port = setting("RT_FTPS_PORT");
    let password = setting("RT_FTP_PASSWORD");
    let scratch = tempfile::tempdir().unwrap();
    let mut known = KnownHosts::load(&scratch.path().join("known_hosts.toml")).unwrap();

    // The first contact stops before the password is sent.
    let Err(Error::UnknownCertificate(Certificate {
        key,
        host,
        port: pinned_port,
        ..
    })) = FtpFs::connect(settings(&port, &password, Some(known.clone()))).await
    else {
        panic!("the first contact should stop at the certificate");
    };
    known.trust(&host, pinned_port, &key).unwrap();

    let files = FtpFs::connect(settings(&port, &password, Some(known.clone())))
        .await
        .unwrap();
    exercise(&files, scratch.path(), "remotething-test-tls").await;

    // A different certificate under the same name is a change, not a new host.
    known
        .trust(&host, pinned_port, "ftps-cert-sha256:00")
        .unwrap();
    let changed = FtpFs::connect(settings(&port, &password, Some(known))).await;
    assert!(matches!(changed, Err(Error::CertificateChanged(_))));
}
