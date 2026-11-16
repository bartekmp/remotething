//! Remote file systems for Remotething: listing folders and moving files.
//!
//! Every backend implements [`RemoteFs`]: SFTP, FTP and FTPS, SMB, and the local disk.

mod entry;
mod fs;
mod ftp;
mod ftp_tls;
mod local;
mod sftp;
mod smb;
mod transfer;

pub use entry::{Entry, EntryKind, join, parent, sort};
pub use fs::{OnProgress, RemoteFs};
pub use ftp::{FtpFs, FtpSettings};
pub use ftp_tls::Certificate;
pub use local::LocalFs;
pub use sftp::SftpFs;
pub use smb::{SmbFs, SmbSettings};
pub use transfer::{Progress, Transferred};

/// Everything that can go wrong with a remote file system.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The server refused the user name or password.
    #[error("the server refused the user name or password")]
    LoginRefused,

    /// The FTPS server's certificate is new; the user decides whether to trust it.
    #[error("the server's certificate is not known yet")]
    UnknownCertificate(Certificate),

    /// The FTPS server's certificate differs from the trusted one.
    #[error("the server's certificate has changed")]
    CertificateChanged(Certificate),

    /// The channel for the file service could not be opened.
    #[error("the server did not start its file service: {0}")]
    Unavailable(String),

    /// The server refused or failed an operation.
    #[error("{action} {path}: {reason}")]
    Remote {
        /// What was being done, such as "cannot list".
        action: &'static str,
        /// The remote path involved.
        path: String,
        /// What the server said.
        reason: String,
    },

    /// A local file could not be read or written.
    #[error("cannot access the local file {path}: {reason}")]
    Local {
        /// The local file involved.
        path: std::path::PathBuf,
        /// What the operating system said.
        reason: String,
    },
}

/// The result type used across this crate.
pub type Result<T> = std::result::Result<T, Error>;
