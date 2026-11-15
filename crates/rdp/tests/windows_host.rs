//! Against a real Windows host. Ignored unless the host is given:
//! `RT_RDP_HOST=192.168.0.35 RT_RDP_USER=... RT_RDP_PASSWORD=... cargo test -p remotething-rdp -- --ignored`

use std::collections::HashSet;
use std::time::{Duration, Instant};

use remotething_rdp::{Button, Error, Event, Input, Target, connect};
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
