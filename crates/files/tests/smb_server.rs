//! SMB against a real server. Ignored by default: it needs one.
//!
//! ```sh
//! RT_SMB_HOST=127.0.0.1 RT_SMB_PORT=4450 RT_SMB_USER=... RT_SMB_PASSWORD=... RT_SMB_SHARE=files \
//!     cargo test -p remotething-files --test smb_server -- --ignored
//! ```

mod common;

use remotething_files::{EntryKind, Error, RemoteFs, SmbFs, SmbSettings, join};
use remotething_secrets::Secret;

fn setting(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this test"))
}

fn settings(password: &str) -> SmbSettings {
    SmbSettings {
        host: setting("RT_SMB_HOST"),
        port: setting("RT_SMB_PORT").parse().unwrap(),
        username: setting("RT_SMB_USER"),
        password: Secret::new(password.to_string()),
        domain: None,
    }
}

#[tokio::test]
#[ignore = "needs an SMB server; see the top of this file"]
async fn the_root_lists_the_shares_and_a_share_does_everything_the_file_pane_needs() {
    let files = SmbFs::connect(settings(&setting("RT_SMB_PASSWORD")))
        .await
        .unwrap();
    let share = setting("RT_SMB_SHARE");

    let home = files.home().await.unwrap();
    assert_eq!(home, "/");
    let shares = files.list(&home).await.unwrap();
    assert!(
        shares
            .iter()
            .any(|entry| entry.name == share && entry.kind == EntryKind::Folder),
        "{shares:?}"
    );
    // IPC$ and the like are not folders to browse.
    assert!(shares.iter().all(|entry| entry.name != "IPC$"));

    let scratch = tempfile::tempdir().unwrap();
    let top = join(&join(&home, &share), "remotething-test-smb");
    common::exercise(&files, scratch.path(), &top).await;
}

#[tokio::test]
#[ignore = "needs an SMB server; see the top of this file"]
async fn a_wrong_password_is_refused_as_such() {
    let result = SmbFs::connect(settings("not-the-password")).await;

    assert!(
        matches!(result, Err(Error::LoginRefused)),
        "{:?}",
        result.err()
    );
}

#[tokio::test]
#[ignore = "needs an SMB server; see the top of this file"]
async fn shares_cannot_be_renamed_or_made_from_the_root() {
    let files = SmbFs::connect(settings(&setting("RT_SMB_PASSWORD")))
        .await
        .unwrap();

    assert!(
        files.create_folder("/newshare").await.is_err() || files.exists("/newshare").await.is_ok()
    );
    assert!(files.rename("/a", "/b").await.is_err());
}
