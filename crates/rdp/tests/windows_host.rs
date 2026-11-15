//! Against a real Windows host. Ignored unless the host is given:
//! `RT_RDP_HOST=192.168.0.35 RT_RDP_USER=... RT_RDP_PASSWORD=... cargo test -p remotething-rdp -- --ignored`

use std::collections::HashSet;
use std::time::{Duration, Instant};

use remotething_rdp::{Button, Error, Event, Input, Quality, Target, connect};
use remotething_secrets::{KnownHosts, Secret};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

fn target(password: &str) -> Target {
    Target {
        host: env("RT_RDP_HOST"),
        port: 3389,
        username: env("RT_RDP_USER"),
        password: Secret::new(password.to_string()),
        domain: None,
        width: 1280,
        height: 800,
        quality: Quality::default(),
        scale: 100,
    }
}

/// A known-hosts file that already trusts the host's certificate.
fn trusting() -> (tempfile::TempDir, KnownHosts) {
    let directory = tempfile::tempdir().unwrap();
    let mut known = KnownHosts::load(&directory.path().join("known_hosts.toml")).unwrap();
    let Err(Error::UnknownCertificate(certificate)) =
        connect(target(&env("RT_RDP_PASSWORD")), &known)
    else {
        panic!("a first connection should stop at the certificate");
    };
    known
        .trust(&certificate.host, certificate.port, &certificate.key)
        .unwrap();
    (directory, known)
}

fn colours(rgba: &[u8]) -> usize {
    rgba.chunks_exact(4).collect::<HashSet<_>>().len()
}

#[test]
#[ignore = "needs RT_RDP_HOST, RT_RDP_USER and RT_RDP_PASSWORD"]
fn a_desktop_arrives_and_input_is_accepted() {
    let (_directory, known) = trusting();
    let mut session = connect(target(&env("RT_RDP_PASSWORD")), &known).unwrap();

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut rich = false;
    while Instant::now() < deadline && !rich {
        if let Some(Event::Frame(frame)) = session.events.blocking_recv() {
            rich = colours(&frame.rgba) > 1000;
        }
    }
    assert!(rich, "no desktop frame within 30 seconds");

    let moved = Input::MouseMoved { x: 400, y: 300 };
    let clicked = Input::Button {
        button: Button::Left,
        pressed: true,
        x: 400,
        y: 300,
    };
    session.input.blocking_send(moved).unwrap();
    session.input.blocking_send(clicked).unwrap();
}

#[test]
#[ignore = "needs RT_RDP_HOST, RT_RDP_USER and RT_RDP_PASSWORD"]
fn a_wrong_password_is_refused_after_the_certificate_was_trusted() {
    let (_directory, known) = trusting();

    let result = connect(target("definitely not the password"), &known);

    assert!(
        matches!(result, Err(Error::AuthRefused)),
        "{:?}",
        result.err()
    );
}

#[test]
#[ignore = "needs RT_RDP_HOST, RT_RDP_USER and RT_RDP_PASSWORD"]
fn the_desktop_follows_a_resize() {
    let (_directory, known) = trusting();
    let mut session = connect(target(&env("RT_RDP_PASSWORD")), &known).unwrap();
    session
        .input
        .blocking_send(Input::Resize {
            width: 1024,
            height: 768,
            scale: 100,
        })
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut sizes = Vec::new();
    while Instant::now() < deadline {
        let event = session.events.blocking_recv();
        if let Some(Event::Closed(reason)) = &event {
            panic!("session closed: {reason:?}, sizes seen: {sizes:?}");
        }
        if let Some(Event::Frame(frame)) = event {
            let size = (frame.width, frame.height);
            if sizes.last() != Some(&size) {
                sizes.push(size);
            }
            if size == (1024, 768) {
                return;
            }
        }
    }
    panic!("no frame of the new size within 30 seconds, sizes seen: {sizes:?}");
}

#[test]
#[ignore = "needs RT_RDP_HOST, RT_RDP_USER and RT_RDP_PASSWORD"]
fn every_quality_preset_gives_a_desktop() {
    let (_directory, known) = trusting();
    for quality in [Quality::Lan, Quality::Balanced, Quality::LowBandwidth] {
        let mut wanted = target(&env("RT_RDP_PASSWORD"));
        wanted.quality = quality;
        let mut session = connect(wanted, &known).unwrap();

        let deadline = Instant::now() + Duration::from_secs(30);
        let mut most = 0;
        while Instant::now() < deadline && most < 8 {
            if let Some(Event::Frame(frame)) = session.events.blocking_recv() {
                most = most.max(colours(&frame.rgba));
            }
        }
        // Without a wallpaper the desktop has few colours, but more than one.
        assert!(
            most >= 8,
            "{quality:?}: only {most} colours within 30 seconds"
        );
        drop(session);
        // The server needs a moment to let go of the old connection.
        std::thread::sleep(Duration::from_secs(3));
    }
}

/// The resident memory of this process in kilobytes, from /proc (Linux only).
fn resident_kilobytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// A long session: input every few seconds and a resize every minute, the way
/// someone works. It must stay up, keep reporting, and not grow in memory.
///
/// `RT_RDP_SOAK_MINUTES` sets the length; the default is 3 minutes. Run it for an
/// hour before a release: `RT_RDP_SOAK_MINUTES=60 cargo test --release -p remotething-rdp --test windows_host -- --ignored --nocapture a_long_session`.
#[test]
#[ignore = "needs RT_RDP_HOST, RT_RDP_USER and RT_RDP_PASSWORD, and takes minutes"]
fn a_long_session_stays_up_and_does_not_grow() {
    let minutes: u64 = std::env::var("RT_RDP_SOAK_MINUTES")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(3);
    let (_directory, known) = trusting();
    // A server that has only just let go of the last session may drop the first attempt.
    let mut session = (0..3)
        .find_map(|attempt| {
            if attempt > 0 {
                std::thread::sleep(Duration::from_secs(5));
            }
            connect(target(&env("RT_RDP_PASSWORD")), &known).ok()
        })
        .expect("the server took none of three attempts");

    let started = Instant::now();
    let end = started + Duration::from_secs(minutes * 60);
    let mut last_input = Instant::now();
    let mut last_resize = Instant::now();
    let mut last_report = Instant::now();
    let mut wide = false;
    let mut frames = 0u64;
    let mut baseline: Option<u64> = None;
    while Instant::now() < end {
        while let Ok(event) = session.events.try_recv() {
            match event {
                Event::Frame(_) => frames += 1,
                Event::Closed(reason) => {
                    panic!("closed after {:?}: {reason:?}", started.elapsed())
                }
                _ => {}
            }
        }
        if last_input.elapsed() >= Duration::from_secs(5) {
            last_input = Instant::now();
            let x = if wide { 300 } else { 500 };
            wide = !wide;
            session
                .input
                .blocking_send(Input::MouseMoved { x, y: 300 })
                .unwrap();
        }
        if last_resize.elapsed() >= Duration::from_secs(60) {
            last_resize = Instant::now();
            let (width, height) = if wide { (1280, 800) } else { (1000, 700) };
            session
                .input
                .blocking_send(Input::Resize {
                    width,
                    height,
                    scale: 100,
                })
                .unwrap();
        }
        // The memory is measured after the first minutes, once everything has settled.
        if baseline.is_none() && started.elapsed() > Duration::from_secs(90) {
            baseline = resident_kilobytes();
        }
        if last_report.elapsed() >= Duration::from_secs(300) {
            last_report = Instant::now();
            eprintln!(
                "SOAK {:?}: {frames} frames, {:?} kB resident",
                started.elapsed(),
                resident_kilobytes()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(frames > 0, "no frame in the whole session");
    if let (Some(before), Some(after)) = (baseline, resident_kilobytes()) {
        let grown = after.saturating_sub(before);
        assert!(
            grown < 150 * 1024,
            "memory grew by {} MB ({before} kB to {after} kB)",
            grown / 1024
        );
    }
}
