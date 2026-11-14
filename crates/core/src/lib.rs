//! The heart of Remotething: what a connection is, how folders pass settings
//! down to it, and how the whole tree is stored on disk.
//!
//! Nothing here touches the network, the keyring or the UI.

mod config;
mod edit;
mod error;
mod inherit;
mod model;
mod validate;

pub use error::Error;
pub use inherit::ResolvedConnection;
pub use model::{
    Connection, Credential, CredentialKind, Folder, Protocol, Quality, RdpSettings, Settings, Store,
};

/// The result type used across this crate.
pub type Result<T> = std::result::Result<T, Error>;
