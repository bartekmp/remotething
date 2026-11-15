//! The heart of Remotething: what a connection is, how folders pass settings
//! down to it, and how the whole tree is stored on disk.
//!
//! Nothing here touches the network, the keyring or the UI.

mod config;
mod edit;
mod error;
mod exchange;
mod forward;
mod inherit;
mod model;
mod putty;
mod quick;
mod validate;

pub use error::Error;
pub use exchange::ImportSummary;
pub use forward::{BadForward, Forward, format_forwards, parse_forwards};
pub use inherit::ResolvedConnection;
pub use model::{
    Connection, Credential, CredentialKind, Folder, Protocol, Quality, RdpSettings, Settings, Store,
};
pub use putty::{
    ImportPlan, PlannedConnection, PuttySession, Skipped, decode_name, decode_reg_bytes, parse_reg,
    parse_session_file, plan as plan_putty_import, unreadable,
};
pub use quick::{QuickTarget, parse_quick};

/// The result type used across this crate.
pub type Result<T> = std::result::Result<T, Error>;
