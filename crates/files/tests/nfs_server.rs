//! NFS against a real server. Ignored by default: it needs one.
//!
//! The export must be writable for the user number given, and allow clients on ports above 1023
//! (`insecure`). For example, with `/srv/test 127.0.0.1(rw,insecure,all_squash,anonuid=1000)` in
//! `/etc/exports`:
//!
//! ```sh
//! RT_NFS_HOST=127.0.0.1 RT_NFS_UID=1000 RT_NFS_EXPORT=/srv/test \
//!     cargo test -p remotething-files --test nfs_server -- --ignored
//! ```

mod common;

use remotething_files::{EntryKind, NfsFs, NfsSettings, RemoteFs, join};

fn setting(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this test"))
}

async fn connect() -> NfsFs {
    let uid: u32 = setting("RT_NFS_UID").parse().unwrap();
    NfsFs::connect(NfsSettings {
        host: setting("RT_NFS_HOST"),
        port: 111,
        uid,
        gid: uid,
    })
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "needs an NFS server; see the top of this file"]
async fn the_root_leads_to_the_export_and_an_export_does_everything_the_file_pane_needs() {
    let files = connect().await;
    let export = setting("RT_NFS_EXPORT");

    // Walk down from the root to the export, through the folders made up on the way.
    let mut here = files.home().await.unwrap();
    for part in export.split('/').filter(|part| !part.is_empty()) {
        let listing = files.list(&here).await.unwrap();
        assert!(
            listing
                .iter()
                .any(|entry| entry.name == part && entry.kind == EntryKind::Folder),
            "{part} is not in {here}: {listing:?}"
        );
        here = join(&here, part);
    }

    let scratch = tempfile::tempdir().unwrap();
    let top = join(&here, "remotething-test-nfs");
    common::exercise(&files, scratch.path(), &top).await;
}
