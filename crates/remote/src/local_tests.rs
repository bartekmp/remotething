//! Tests of `local.rs`, with the shell of the machine the tests run on.

use std::time::Duration;

use tokio::time::timeout;

use super::*;

const SIZE: TerminalSize = TerminalSize {
    columns: 80,
    lines: 24,
};

/// Everything printed until the program ends, and its exit status.
async fn until_closed(events: &mut mpsc::Receiver<ShellEvent>) -> (String, Option<u32>) {
    let mut printed = Vec::new();
    loop {
        match timeout(Duration::from_secs(10), events.recv()).await {
            Ok(Some(ShellEvent::Output(bytes))) => printed.extend(bytes),
            Ok(Some(ShellEvent::Closed(status))) => {
                return (String::from_utf8_lossy(&printed).into_owned(), status);
            }
            other => panic!("the program did not end: {other:?}"),
        }
    }
}

fn shell(script: &str) -> LocalShell {
    open("/bin/sh", &["-c".to_string(), script.to_string()], SIZE).unwrap()
}

#[tokio::test]
async fn what_the_program_prints_arrives_and_then_its_exit_status() {
    let mut running = shell("echo hello; exit 3");

    let (printed, status) = until_closed(&mut running.events).await;

    assert!(printed.contains("hello"), "{printed:?}");
    assert_eq!(status, Some(3));
}

#[tokio::test]
async fn typed_bytes_reach_the_program() {
    let mut running = open("/bin/cat", &[], SIZE).unwrap();

    running
        .input
        .send(ShellInput::Data(b"ping\n".to_vec()))
        .await
        .unwrap();
    let mut printed = String::new();
    while !printed.contains("ping") {
        match timeout(Duration::from_secs(10), running.events.recv()).await {
            Ok(Some(ShellEvent::Output(bytes))) => {
                printed.push_str(&String::from_utf8_lossy(&bytes))
            }
            other => panic!("no echo: {other:?} after {printed:?}"),
        }
    }
}

#[tokio::test]
async fn the_terminal_has_the_size_asked_for_and_can_change_it() {
    let mut running = shell("stty size; read line; stty size");

    // The first size is printed before anything is changed.
    let mut printed = String::new();
    while !printed.contains("24 80") {
        match timeout(Duration::from_secs(10), running.events.recv()).await {
            Ok(Some(ShellEvent::Output(bytes))) => {
                printed.push_str(&String::from_utf8_lossy(&bytes))
            }
            other => panic!("no size: {other:?} after {printed:?}"),
        }
    }
    running
        .input
        .send(ShellInput::Resize(TerminalSize {
            columns: 100,
            lines: 30,
        }))
        .await
        .unwrap();
    // Input is handled in order, so the resize is done before the line is read.
    running
        .input
        .send(ShellInput::Data(b"go\n".to_vec()))
        .await
        .unwrap();
    let (rest, _) = until_closed(&mut running.events).await;

    assert!(rest.contains("30 100"), "{rest:?}");
}

#[tokio::test]
async fn dropping_the_input_stops_the_program() {
    let LocalShell { mut events, input } = shell("sleep 60");

    drop(input);
    let (_, status) = until_closed(&mut events).await;

    // Killed, so there is no clean exit status of 0.
    assert_ne!(status, Some(0));
}

#[tokio::test]
async fn a_program_that_does_not_exist_is_an_error_not_a_hang() {
    let result = open("/definitely/not/a/program", &[], SIZE);

    assert!(result.is_err());
}
