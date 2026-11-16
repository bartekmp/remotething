//! The protocols Remotething speaks.
//!
//! Each protocol is a module behind a Cargo feature. Nothing here knows
//! about the UI: a session takes input over a channel and sends events back.

#[cfg(feature = "local")]
pub mod local;
#[cfg(feature = "ssh")]
pub mod ssh;
