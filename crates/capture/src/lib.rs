//! Writers for recordings of sessions.
//!
//! [`asciicast`] writes terminal sessions in the asciicast v2 format that
//! `asciinema play` reads, [`png`] writes pictures and [`avi`] writes motion JPEG
//! videos whose frames [`jpeg`] makes.

pub mod asciicast;
pub mod avi;
pub mod jpeg;
pub mod png;
mod utf8;

pub use utf8::Utf8Splitter;
