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

#[tokio::test]
#[ignore = "needs RT_VNC_HOST and RT_VNC_PASSWORD"]
async fn the_server_sends_a_cursor_shape() {
    let target = Target {
        host: env("RT_VNC_HOST"),
        port: 5900,
        password: Some(Secret::new(env("RT_VNC_PASSWORD"))),
        quality: Quality::default(),
    };
    let mut session = connect(target).await.unwrap();
    let mut shapes = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut step = 0u16;
    while Instant::now() < deadline {
        // Moving the pointer makes a server that was waiting send its cursor.
        step += 7;
        let _ = session
            .input
            .send(remotething_vnc::Input::MouseMoved {
                x: 200 + step,
                y: 200 + step / 2,
            })
            .await;
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(500), session.events.recv()).await
        {
            match event {
                Event::Cursor(Some(cursor)) => {
                    let visible = cursor
                        .rgba
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|p| p[3] > 0)
                        .count();
                    shapes.push(format!(
                        "{}x{} hot {},{} visible pixels {}",
                        cursor.width, cursor.height, cursor.hotspot_x, cursor.hotspot_y, visible
                    ));
                }
                Event::Cursor(None) => shapes.push("hidden".to_string()),
                _ => {}
            }
        }
    }
    eprintln!("CURSOR EVENTS: {shapes:?}");
    assert!(!shapes.is_empty(), "the server sent no cursor shape");
}
