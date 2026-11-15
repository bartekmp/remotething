//! Windows Remote Desktop sessions over IronRDP.
//!
//! [`connect`] checks the server's certificate against the pinned ones before
//! any credential leaves the machine, then runs the session on its own thread.
//! Frames come out of [`Session::events`], input goes into [`Session::input`].

mod clipboard;
mod error;
mod input;
mod quality;
mod reactivate;
mod session;
mod settings;
mod stats;
mod tls;
mod worker;

pub use error::{Certificate, Error, Result};
pub use input::{Button, Input, ctrl_alt_delete};
pub use quality::Quality;
pub use session::{Event, Frame, Info, Session, Target, connect};
pub use stats::Stats;
