//! Writers for recordings of sessions.
//!
//! [`asciicast`] writes terminal sessions in the asciicast v2 format that
//! `asciinema play` reads.

pub mod asciicast;
mod utf8;

pub use utf8::Utf8Splitter;
