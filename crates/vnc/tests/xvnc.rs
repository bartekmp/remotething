//! Against a real TigerVNC server (`Xvnc`) started for each test. A test is
//! skipped, with a note, when `Xvnc` or `vncpasswd` is not installed.

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use remotething_secrets::Secret;
use remotething_vnc::{Button, Error, Event, Input, Quality, Target, connect};

const PASSWORD: &str = "open sesame";

/// Keeps ports apart between tests, which run side by side: in one process (`cargo test`)
/// by counting up, and between processes (`cargo nextest`, one process per test) by a
/// starting point that depends on the process.
static NEXT_PORT: LazyLock<AtomicU16> = LazyLock::new(|| {
    let slot = u16::try_from(std::process::id() % 1000).expect("below a thousand");
    AtomicU16::new(5980 + slot * 8)
});

/// A running server, stopped when dropped.
struct Server {
    child: Child,
    port: u16,
    _directory: tempfile::TempDir,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn installed(program: &str) -> bool {
    Command::new(program)
        .arg("-h")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// The password file of `vncpasswd`, which wants the password on its input.
fn password_file(directory: &std::path::Path) -> std::path::PathBuf {
    let mut child = Command::new("vncpasswd")
        .arg("-f")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("vncpasswd starts");
    child
        .stdin
        .take()
        .expect("piped")
        .write_all(format!("{PASSWORD}\n").as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let path = directory.join("passwd");
    std::fs::write(&path, output.stdout).unwrap();
    path
}

fn start(security: &str) -> Option<Server> {
    if !installed("Xvnc") || !installed("vncpasswd") {
        eprintln!("skipped: Xvnc or vncpasswd is not installed");
        return None;
    }
    let directory = tempfile::tempdir().unwrap();
    let passwords = password_file(directory.path());
    let port = NEXT_PORT.fetch_add(1, Ordering::Relaxed);
    let child = Command::new("Xvnc")
        .arg(format!(":{}", port + 100))
        .args(["-rfbport", &port.to_string()])
        .args(["-geometry", "640x480"])
        .args(["-SecurityTypes", security])
        .arg("-rfbauth")
        .arg(&passwords)
        .args(["-localhost", "yes"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Xvnc starts");
    let server = Server {
        child,
        port,
        _directory: directory,
    };
    // The server needs a moment before it takes connections.
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Some(server);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("the server did not start");
}

fn target(server: &Server, password: Option<&str>) -> Target {
    Target {
        host: "127.0.0.1".to_string(),
        port: server.port,
        password: password.map(|text| Secret::new(text.to_string())),
        quality: Quality::default(),
    }
}

#[tokio::test]
async fn a_picture_of_the_right_size_arrives_and_input_is_accepted() {
    let Some(server) = start("VncAuth") else {
        return;
    };
    let mut session = connect(target(&server, Some(PASSWORD))).await.unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut size = None;
    while Instant::now() < deadline && size.is_none() {
        if let Ok(Some(Event::Frame(frame))) =
            tokio::time::timeout(Duration::from_secs(1), session.events.recv()).await
            && frame.width > 1
        {
            size = Some((frame.width, frame.height));
        }
    }
    assert_eq!(size, Some((640, 480)));

    let moved = Input::MouseMoved { x: 100, y: 100 };
    let clicked = Input::Button {
        button: Button::Left,
        pressed: true,
        x: 100,
        y: 100,
    };
    session.input.send(moved).await.unwrap();
    session.input.send(clicked).await.unwrap();
    session
        .input
        .send(Input::Key {
            keysym: 0x61,
            pressed: true,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_wrong_password_is_refused() {
    let Some(server) = start("VncAuth") else {
        return;
    };

    let result = connect(target(&server, Some("not it"))).await;

    assert!(
        matches!(result, Err(Error::WrongPassword)),
        "{:?}",
        result.err()
    );
}

#[tokio::test]
async fn a_missing_password_is_asked_for() {
    let Some(server) = start("VncAuth") else {
        return;
    };

    let result = connect(target(&server, None)).await;

    assert!(
        matches!(result, Err(Error::PasswordNeeded)),
        "{:?}",
        result.err()
    );
}

#[tokio::test]
async fn a_server_that_is_not_there_is_reported() {
    let result = connect(Target {
        host: "127.0.0.1".to_string(),
        port: 1,
        password: None,
        quality: Quality::default(),
    })
    .await;

    assert!(matches!(result, Err(Error::Unreachable { .. })));
}

#[tokio::test]
async fn the_low_bandwidth_preset_still_gives_the_right_picture() {
    let Some(server) = start("VncAuth") else {
        return;
    };
    let mut wanted = target(&server, Some(PASSWORD));
    wanted.quality = Quality::LowBandwidth;
    let mut session = connect(wanted).await.unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut size = None;
    while Instant::now() < deadline && size.is_none() {
        if let Ok(Some(Event::Frame(frame))) =
            tokio::time::timeout(Duration::from_secs(1), session.events.recv()).await
            && frame.width > 1
        {
            size = Some((frame.width, frame.height));
        }
    }

    assert_eq!(session.info.colours, "16 bit");
    assert_eq!(size, Some((640, 480)));
}

#[tokio::test]
async fn a_server_that_can_resize_follows_the_requested_size_without_being_told_it_can() {
    let Some(server) = start("VncAuth") else {
        return;
    };
    let mut session = connect(target(&server, Some(PASSWORD))).await.unwrap();

    // Nothing is known about the server; the request is made and the server decides.
    session
        .input
        .send(Input::Resize {
            width: 800,
            height: 600,
        })
        .await
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut size = None;
    while Instant::now() < deadline && size != Some((800, 600)) {
        if let Ok(Some(Event::Frame(frame))) =
            tokio::time::timeout(Duration::from_secs(1), session.events.recv()).await
        {
            size = Some((frame.width, frame.height));
        }
        // A request before the server's first announcement is not asked of it; try again.
        let _ = session
            .input
            .send(Input::Resize {
                width: 800,
                height: 600,
            })
            .await;
    }
    assert_eq!(size, Some((800, 600)));
}
