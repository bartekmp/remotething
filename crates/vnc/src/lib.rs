//! VNC sessions over vnc-rs.
//!
//! [`connect`] reaches a server and starts the session on a tokio task.
//! Pictures and the pointer come out of [`Session::events`], input goes into
//! [`Session::input`]. [`connect_over`] takes any byte stream, for example a
//! channel through an SSH connection.

mod error;
mod framebuffer;
mod input;
mod quality;
mod session;
mod traffic;

pub use error::{Error, Result};
pub use input::{Button, Input};
pub use quality::Quality;
pub use session::{Cursor, Event, Frame, Info, Session, Stats, Target, connect, connect_over};
