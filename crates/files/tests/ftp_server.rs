//! FTP and FTPS against a real server. Ignored by default: it needs one.
//!
//! ```sh
//! RT_FTP_HOST=127.0.0.1 RT_FTP_PORT=2121 RT_FTP_USER=test RT_FTP_PASSWORD=... \
//!     cargo test -p remotething-files --test ftp_server -- --ignored
//! ```
//!
//! Set `RT_FTPS_PORT` too to run the FTPS test against a server with TLS.

mod common;

use remotething_files::{Certificate, Error, FtpFs, FtpSettings, RemoteFs, join};
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

    let top = join(&files.home().await.unwrap(), "remotething-test-plain");
    common::exercise(&files, scratch.path(), &top).await;
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
    let top = join(&files.home().await.unwrap(), "remotething-test-tls");
    common::exercise(&files, scratch.path(), &top).await;

    // A different certificate under the same name is a change, not a new host.
    known
        .trust(&host, pinned_port, "ftps-cert-sha256:00")
        .unwrap();
    let changed = FtpFs::connect(settings(&port, &password, Some(known))).await;
    assert!(matches!(changed, Err(Error::CertificateChanged(_))));
}
