//! Against a real VNC server on Windows (TightVNC and the like). Ignored unless given:
//! `RT_VNC_HOST=192.168.0.35 RT_VNC_PASSWORD=... cargo test -p remotething-vnc --test windows_vnc -- --ignored`

use std::time::{Duration, Instant};

use remotething_secrets::Secret;
use remotething_vnc::{Event, Quality, Target, connect};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

#[tokio::test]
#[ignore = "needs RT_VNC_HOST and RT_VNC_PASSWORD"]
async fn a_tight_server_signs_in_with_its_password_and_sends_a_picture() {
    let target = Target {
        host: env("RT_VNC_HOST"),
        port: std::env::var("RT_VNC_PORT")
            .map(|port| port.parse().unwrap())
            .unwrap_or(5900),
        password: Some(Secret::new(env("RT_VNC_PASSWORD"))),
        quality: Quality::default(),
    };

    let mut session = connect(target)
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), session.events.recv()).await {
            Ok(Some(Event::Frame(frame))) => {
                assert!(frame.width > 100 && frame.height > 100);
                return;
            }
            Ok(Some(Event::Closed(reason))) => panic!("closed: {reason:?}"),
            Ok(Some(_)) | Err(_) => {}
            Ok(None) => panic!("session ended"),
        }
    }
    panic!("no picture within 20 seconds");
}
