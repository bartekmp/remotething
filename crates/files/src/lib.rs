//! Remote file systems for Remotething: listing folders and moving files.
//!
//! SFTP is the only backend so far. A shared trait for backends comes with the second one.

mod entry;
mod sftp;

pub use entry::{Entry, EntryKind, join, parent, sort};
pub use sftp::SftpFs;

/// Everything that can go wrong with a remote file system.
#[derive(Debug, thiserror::Error)]
pub enum Error {
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
