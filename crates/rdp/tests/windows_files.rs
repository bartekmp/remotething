//! Files over the clipboard channel against a real Windows host. Ignored unless the
//! host is given, like `windows_host.rs`. It types into the host's Run dialog, so the
//! desktop must be signed in and idle, and it leaves `C:\rtrecv` behind.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{env, target, trusting};
use remotething_rdp::{Direction, Event, FilesEvent, Input, RemoteFile, Session, connect};

const LEFT_SHIFT: u8 = 0x2A;
const CTRL: u8 = 0x1D;
const ENTER: u8 = 0x1C;
const WINDOWS: u8 = 0x5B;

/// The set-1 scancode of a character, and whether it needs Shift. Only what the commands use.
fn scancode(character: char) -> (u8, bool) {
    let letters = "qwertyuiop";
    let middle = "asdfghjkl";
    let bottom = "zxcvbnm";
    let lower = character.to_ascii_lowercase();
    let shift = character.is_ascii_uppercase();
    if let Some(at) = letters.find(lower) {
        return (0x10 + at as u8, shift);
    }
    if let Some(at) = middle.find(lower) {
        return (0x1E + at as u8, shift);
    }
    if let Some(at) = bottom.find(lower) {
        return (0x2C + at as u8, shift);
    }
    match character {
        '1'..='9' => (0x02 + (character as u8 - b'1'), false),
        '0' => (0x0B, false),
        '-' => (0x0C, false),
        ' ' => (0x39, false),
        '\\' => (0x2B, false),
        '.' => (0x34, false),
        ',' => (0x33, false),
        '/' => (0x35, false),
        ':' => (0x27, true),
        '*' => (0x09, true),
        other => panic!("no scancode for {other:?}"),
    }
}

fn press(session: &Session, code: u8, extended: bool, pressed: bool) {
    let key = Input::Key {
        code,
        extended,
        pressed,
    };
    session.input.blocking_send(key).unwrap();
    std::thread::sleep(Duration::from_millis(30));
}

fn tap(session: &Session, code: u8, extended: bool) {
    press(session, code, extended, true);
    press(session, code, extended, false);
}

fn type_text(session: &Session, text: &str) {
    for character in text.chars() {
        let (code, shift) = scancode(character);
        if shift {
            press(session, LEFT_SHIFT, false, true);
        }
        tap(session, code, false);
        if shift {
            press(session, LEFT_SHIFT, false, false);
        }
    }
}

/// Runs a command through Win+R, and gives it a moment to start.
fn run(session: &Session, command: &str) {
    press(session, WINDOWS, true, true);
    tap(session, 0x13, false);
    press(session, WINDOWS, true, false);
    std::thread::sleep(Duration::from_millis(1500));
    type_text(session, command);
    tap(session, ENTER, false);
    std::thread::sleep(Duration::from_millis(3000));
}

/// Reads events until `wanted` picks one, panicking after `seconds`. Frames are skipped.
fn wait_for<T>(
    session: &mut Session,
    seconds: u64,
    what: &str,
    mut wanted: impl FnMut(&FilesEvent) -> Option<T>,
) -> T {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        match session.events.blocking_recv() {
            Some(Event::Files(event)) => {
                if let Some(found) = wanted(&event) {
                    return found;
                }
                seen.push(format!("{event:?}"));
            }
            Some(Event::Closed(reason)) => panic!("session closed: {reason:?}"),
            Some(Event::ConnectionLost(reason)) => panic!("connection lost: {reason}"),
            Some(_) => {}
            None => panic!("session ended"),
        }
    }
    panic!("{what} did not happen within {seconds} seconds; file events seen: {seen:?}");
}

/// Copies files on the host with PowerShell and receives them into a fresh folder.
fn receive(
    session: &mut Session,
    paths: &str,
    expected: usize,
) -> (tempfile::TempDir, Vec<RemoteFile>) {
    run(
        session,
        &format!("powershell -NoProfile -Command Set-Clipboard -Path {paths}"),
    );
    let list = wait_for(
        session,
        30,
        "the host's files being announced",
        |event| match event {
            FilesEvent::Available(list) if list.len() >= expected => Some(list.clone()),
            _ => None,
        },
    );
    let folder = tempfile::tempdir().unwrap();
    session
        .input
        .blocking_send(Input::ReceiveFiles(folder.path().to_path_buf()))
        .unwrap();
    wait_for(
        session,
        120,
        "the files being received",
        |event| match event {
            FilesEvent::Finished {
                direction: Direction::Received,
                ..
            } => Some(()),
            FilesEvent::Failed { reason, .. } => panic!("receiving failed: {reason}"),
            _ => None,
        },
    );
    (folder, list)
}

fn bytes(seed: u8, length: usize) -> Vec<u8> {
    (0..length)
        .map(|n| (n as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

#[test]
#[ignore = "needs RT_RDP_HOST, RT_RDP_USER and RT_RDP_PASSWORD, and types into the host"]
fn files_from_the_host_arrive_whole() {
    let (_directory, known) = trusting();
    let mut session = connect(target(&env("RT_RDP_PASSWORD")), &known).unwrap();
    std::thread::sleep(Duration::from_secs(8));

    let (folder, list) = receive(
        &mut session,
        r"C:\Windows\win.ini,C:\Windows\System32\drivers\etc\hosts",
        2,
    );

    for file in list.iter().filter(|file| !file.is_folder) {
        let local = std::fs::read(folder.path().join(&file.path)).unwrap();
        assert_eq!(local.len() as u64, file.size, "{}", file.path);
        assert!(!local.is_empty(), "{}", file.path);
    }
    assert!(folder.path().join("win.ini").exists());
}

#[test]
#[ignore = "needs RT_RDP_HOST, RT_RDP_USER and RT_RDP_PASSWORD, and types into the host"]
fn files_sent_to_the_host_come_back_the_same() {
    let (_directory, known) = trusting();
    let mut session = connect(target(&env("RT_RDP_PASSWORD")), &known).unwrap();
    std::thread::sleep(Duration::from_secs(8));

    // What is sent: a binary file bigger than one chunk, a text file, and a folder with a nested one.
    let sources = tempfile::tempdir().unwrap();
    let root = sources.path();
    let originals: Vec<(&str, Vec<u8>)> = vec![
        ("a.bin", bytes(1, 700_000)),
        ("b.txt", "zażółć gęślą jaźń\n".repeat(50).into_bytes()),
        ("dir/c.txt", bytes(3, 1_000)),
        ("dir/deep/d.bin", bytes(4, 300_000)),
    ];
    for (name, content) in &originals {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    // A clean folder on the host to paste into, shown in Explorer.
    run(&session, r"cmd /c rmdir /s /q C:\rtrecv");
    run(&session, r"cmd /c mkdir C:\rtrecv");
    session
        .input
        .blocking_send(Input::OfferFiles(vec![
            root.join("a.bin"),
            root.join("b.txt"),
            root.join("dir"),
        ]))
        .unwrap();
    wait_for(
        &mut session,
        20,
        "the files being offered",
        |event| match event {
            FilesEvent::Offered { files: 4, .. } => Some(()),
            FilesEvent::Failed { reason, .. } => panic!("offering failed: {reason}"),
            _ => None,
        },
    );
    // Explorer opens over the desktop and takes the paste. The clipboard must not be
    // touched meanwhile, so nothing else is run until it has finished.
    press(&session, WINDOWS, true, true);
    tap(&session, 0x13, false);
    press(&session, WINDOWS, true, false);
    std::thread::sleep(Duration::from_millis(1500));
    type_text(&session, r"explorer C:\rtrecv");
    tap(&session, ENTER, false);
    std::thread::sleep(Duration::from_secs(5));
    press(&session, CTRL, false, true);
    tap(&session, 0x2F, false);
    press(&session, CTRL, false, false);
    wait_for(
        &mut session,
        120,
        "the host taking the files",
        |event| match event {
            FilesEvent::Finished {
                direction: Direction::Sent,
                ..
            } => Some(()),
            _ => None,
        },
    );
    std::thread::sleep(Duration::from_secs(3));

    // Bring them back from the host's folder and compare every byte.
    let (back, _) = receive(
        &mut session,
        r"C:\rtrecv\a.bin,C:\rtrecv\b.txt,C:\rtrecv\dir",
        4,
    );
    for (name, content) in &originals {
        let path: PathBuf = Path::new(back.path()).join(name);
        let received = std::fs::read(&path).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(&received, content, "{name} differs");
    }
}
