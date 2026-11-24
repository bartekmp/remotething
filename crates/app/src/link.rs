//! Opening a web address in the user's browser.

use std::process::Command;

/// Where the project lives.
pub const REPOSITORY: &str = "https://github.com/bartekmp/remotething";

/// Hands the address to the system. A failure is ignored: there is nothing to do about it.
pub fn open(address: &str) {
    let _ = command(address).spawn();
}

#[cfg(target_os = "linux")]
fn command(address: &str) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(address);
    command
}

#[cfg(target_os = "windows")]
fn command(address: &str) -> Command {
    // Explorer opens an address with the default browser and shows no console.
    let mut command = Command::new("explorer");
    command.arg(address);
    command
}

#[cfg(target_os = "macos")]
fn command(address: &str) -> Command {
    let mut command = Command::new("open");
    command.arg(address);
    command
}
