//! Windows Remote Desktop sessions over IronRDP.
//!
//! [`connect`] checks the server's certificate against the pinned ones before
//! any credential leaves the machine, then runs the session on its own thread.
//! Frames come out of [`Session::events`], input goes into [`Session::input`].

mod clipboard;
mod cursor;
mod dib;
mod error;
mod files;
mod files_receive;
mod input;
mod quality;
mod reactivate;
mod session;
mod settings;
mod stats;
mod tls;
mod worker;

pub use cursor::{Cursor, CursorShape};
pub use dib::Picture;
pub use error::{Certificate, Error, Result};
pub use files::{Direction, FilesEvent, RemoteFile};
pub use input::{Button, Input, ctrl_alt_delete};
pub use quality::Quality;
pub use session::{Event, Frame, Info, Session, Target, connect};
pub use stats::Stats;
