//! A program on this machine, such as `wsl.exe -d Ubuntu`, in a pseudo-terminal.
//!
//! It speaks the events of an SSH shell, so a terminal tab treats both alike.
//! On Windows the pseudo-terminal is ConPTY, the one Windows Terminal uses.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::sync::mpsc;

use crate::ssh::{ShellEvent, ShellInput, TerminalSize};

/// How many events or inputs may wait before the sender is slowed down.
const QUEUE: usize = 64;
/// How much output is read at a time.
const READ_SIZE: usize = 16 * 1024;

/// A running program.
pub struct LocalShell {
    /// Output and the end of the program, in order.
    pub events: mpsc::Receiver<ShellEvent>,
    /// Typed bytes and resizes. Dropping it ends the program.
    pub input: mpsc::Sender<ShellInput>,
}

/// The pseudo-terminal, which is let go of when the program ends so the reader sees the end.
type SharedMaster = Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>;

fn size_of(size: TerminalSize) -> PtySize {
    PtySize {
        rows: size.lines,
        cols: size.columns,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Starts `program` with `arguments` in a terminal of `size`.
pub fn open(program: &str, arguments: &[String], size: TerminalSize) -> Result<LocalShell, String> {
    let pair = native_pty_system()
        .openpty(size_of(size))
        .map_err(|error| error.to_string())?;
    let mut command = CommandBuilder::new(program);
    command.args(arguments);
    command.env("TERM", "xterm-256color");
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| format!("{program}: {error}"))?;
    // Only the program holds the other end now, so the reader sees the end when it exits.
    drop(pair.slave);
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| error.to_string())?;
    let mut writer = pair
        .master
        .take_writer()
        .map_err(|error| error.to_string())?;
    let mut killer = child.clone_killer();
    let master: SharedMaster = Arc::new(Mutex::new(Some(pair.master)));

    let (events_tx, events) = mpsc::channel(QUEUE);
    let (input, mut input_rx) = mpsc::channel(QUEUE);

    // What the program prints.
    let output_tx = events_tx.clone();
    let reading = std::thread::spawn(move || {
        let mut buffer = vec![0u8; READ_SIZE];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    let chunk = buffer[..read].to_vec();
                    if output_tx.blocking_send(ShellEvent::Output(chunk)).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // What the program is typed, and the size of the terminal. When the other end of the
    // channel is dropped the program is stopped.
    let resizing = Arc::clone(&master);
    std::thread::spawn(move || {
        while let Some(input) = input_rx.blocking_recv() {
            let done = match input {
                ShellInput::Data(bytes) => writer.write_all(&bytes).and_then(|()| writer.flush()),
                ShellInput::Resize(size) => {
                    resize(&resizing, size);
                    Ok(())
                }
            };
            if done.is_err() {
                break;
            }
        }
        let _ = killer.kill();
    });

    // The end of the program: its output is finished first, then the end is reported.
    std::thread::spawn(move || {
        let status = child.wait().ok().map(|status| status.exit_code());
        // Some systems keep the reader waiting until the terminal is let go of.
        if let Ok(mut held) = master.lock() {
            held.take();
        }
        let _ = reading.join();
        let _ = events_tx.blocking_send(ShellEvent::Closed(status));
    });

    Ok(LocalShell { events, input })
}

fn resize(master: &SharedMaster, size: TerminalSize) {
    if let Ok(held) = master.lock()
        && let Some(master) = held.as_ref()
    {
        let _ = master.resize(size_of(size));
    }
}

#[cfg(test)]
#[path = "local_tests.rs"]
mod tests;
