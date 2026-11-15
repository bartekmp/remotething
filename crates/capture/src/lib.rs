//! Writers for recordings of sessions.
//!
//! [`asciicast`] writes terminal sessions in the asciicast v2 format that
//! `asciinema play` reads, and [`png`] writes pictures.

pub mod asciicast;
pub mod png;
mod utf8;

pub use utf8::Utf8Splitter;
