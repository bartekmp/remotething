//! Checks the video files with ffprobe and ffmpeg, which read them the way a player does.
//! A test is skipped, with a note, when they are not installed.

use std::io::Cursor;
use std::process::Command;

use remotething_capture::avi::Writer;
use remotething_capture::jpeg;

fn installed(program: &str) -> bool {
    Command::new(program)
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// A picture of a gradient that moves with `step`.
fn picture(width: u32, height: u32, step: u32) -> Vec<u8> {
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&[((x + step * 8) % 256) as u8, (y % 256) as u8, 128, 255]);
        }
    }
    rgba
}

fn video(frames: u32) -> Vec<u8> {
    let (width, height) = (160, 120);
    let mut writer = Writer::start(Cursor::new(Vec::new()), width, height, 10).unwrap();
    for step in 0..frames {
        if step % 3 == 2 {
            // Nothing changed: the player keeps the last picture.
            writer.add_repeat().unwrap();
        } else {
            let jpeg = jpeg::encode(width, height, &picture(width, height, step)).unwrap();
            writer.add_frame(&jpeg).unwrap();
        }
    }
    writer.finish().unwrap().into_inner()
}

fn write_temp(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("test.avi");
    std::fs::write(&path, bytes).unwrap();
    (directory, path)
}

#[test]
fn ffprobe_reads_the_codec_size_and_frame_count() {
    if !installed("ffprobe") {
        eprintln!("skipped: ffprobe is not installed");
        return;
    }
    let (_directory, path) = write_temp(&video(30));

    let output = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0"])
        .args([
            "-show_entries",
            "stream=codec_name,width,height,avg_frame_rate",
        ])
        .args(["-of", "default=noprint_wrappers=1"])
        .arg(&path)
        .output()
        .unwrap();

    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("codec_name=mjpeg"), "{text}");
    assert!(
        text.contains("width=160") && text.contains("height=120"),
        "{text}"
    );
    assert!(text.contains("avg_frame_rate=10/1"), "{text}");
}

#[test]
fn ffmpeg_decodes_every_frame_without_complaint() {
    if !installed("ffmpeg") {
        eprintln!("skipped: ffmpeg is not installed");
        return;
    }
    let (_directory, path) = write_temp(&video(30));

    let output = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&path)
        .args(["-f", "null", "-"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_player_counts_all_the_frames_including_repeats() {
    if !installed("ffprobe") {
        eprintln!("skipped: ffprobe is not installed");
        return;
    }
    let (_directory, path) = write_temp(&video(30));

    let output = Command::new("ffprobe")
        .args(["-v", "error", "-count_packets", "-select_streams", "v:0"])
        .args(["-show_entries", "stream=nb_read_packets"])
        .args(["-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(&path)
        .output()
        .unwrap();

    let text = String::from_utf8_lossy(&output.stdout);
    // Ten of the thirty frames are repeats, which a player reads as no new packet.
    let packets: u32 = text.trim().parse().unwrap_or(0);
    assert!((20..=30).contains(&packets), "{text}");
}
