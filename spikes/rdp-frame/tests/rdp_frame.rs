//! Needs the `rdp` service from `spikes/servers/compose.yml`.
//! Run with: cargo test -- --ignored

use std::collections::HashSet;
use std::sync::mpsc;
use std::time::Duration;

use spike_rdp_frame::session::{Frame, Target, run};

fn distinct_colours(frame: &Frame) -> usize {
    let colours: HashSet<&[u8]> = frame.rgba.chunks_exact(4).collect();
    colours.len()
}

#[test]
#[ignore = "needs the Docker xrdp server"]
fn a_real_desktop_frame_arrives() {
    let target = Target {
        host: "127.0.0.1".into(),
        port: 3389,
        username: "abc".into(),
        password: "abc".into(),
        nla: false,
        insecure: false,
    };
    let (done, finished) = mpsc::channel();

    std::thread::spawn(move || {
        let result = run(target, |frame| {
            // A blank or single-colour screen is not proof of decoding.
            if distinct_colours(&frame) < 16 {
                return true;
            }
            let _ = done.send((frame.width, frame.height, frame.rgba.len()));
            false
        });
        result.expect("RDP session failed");
    });

    let (width, height, bytes) = finished
        .recv_timeout(Duration::from_secs(30))
        .expect("no desktop frame within 30 seconds");
    assert_eq!((width, height), (1280, 800));
    assert_eq!(bytes, 1280 * 800 * 4);
}
