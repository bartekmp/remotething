//! What a previewed remote file turns into: text, a picture, or the reason it
//! cannot be shown. Everything is held in memory and nothing is written to disk.

use std::io::Cursor;

use iced::widget::image::Handle;
use image::{ImageReader, Limits};

/// How much of a text file is fetched.
pub(crate) const TEXT_LIMIT: u64 = 256 * 1024;
/// The largest picture that is fetched.
pub(crate) const IMAGE_LIMIT: u64 = 8 * 1024 * 1024;
/// A decoded picture may not need more memory than this, whatever its file size.
const DECODED_LIMIT: u64 = 128 * 1024 * 1024;
/// How much of the start is checked for the zero bytes that mark a binary file.
const BINARY_CHECK: usize = 8 * 1024;

/// What a file turned into.
#[derive(Debug, Clone)]
pub(crate) enum Content {
    /// Text, and whether only its start is shown.
    Text {
        text: String,
        cut: bool,
    },
    Picture(Handle),
    /// A picture bigger than [`IMAGE_LIMIT`].
    TooLarge,
    /// Neither text nor a picture of a known format.
    Binary,
    /// A picture whose file could not be decoded.
    Broken(String),
}

/// Whether the file name says it is a picture of a format that can be shown.
pub(crate) fn is_picture(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["png", "jpg", "jpeg", "gif", "bmp"]
        .iter()
        .any(|extension| {
            lower
                .rsplit_once('.')
                .is_some_and(|(_, found)| found == *extension)
        })
}

/// How many bytes to fetch for a file of this name.
pub(crate) fn limit_for(name: &str) -> u64 {
    if is_picture(name) {
        IMAGE_LIMIT
    } else {
        TEXT_LIMIT
    }
}

/// Turns the fetched start of a file into something to show. `cut` says the file goes on.
pub(crate) fn decode(name: &str, bytes: &[u8], cut: bool) -> Content {
    if is_picture(name) {
        if cut {
            return Content::TooLarge;
        }
        return match picture(bytes, DECODED_LIMIT) {
            Ok(handle) => Content::Picture(handle),
            Err(reason) => Content::Broken(reason),
        };
    }
    if bytes.iter().take(BINARY_CHECK).any(|byte| *byte == 0) {
        return Content::Binary;
    }
    Content::Text {
        text: String::from_utf8_lossy(bytes).into_owned(),
        cut,
    }
}

fn picture(bytes: &[u8], decoded_limit: u64) -> Result<Handle, String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(decoded_limit);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| error.to_string())?;
    let rgba = decoded.to_rgba8();
    Ok(Handle::from_rgba(
        rgba.width(),
        rgba.height(),
        rgba.into_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&vec![200u8; (width * height * 4) as usize])
            .unwrap();
        writer.finish().unwrap();
        out
    }

    #[test]
    fn pictures_are_told_by_their_extension_in_any_case() {
        assert!(is_picture("a.PNG"));
        assert!(is_picture("photo.final.jpeg"));
        assert!(!is_picture("notes.txt"));
        assert!(!is_picture("png"));
    }

    #[test]
    fn a_picture_is_fetched_with_a_larger_limit_than_text() {
        assert_eq!(limit_for("a.png"), IMAGE_LIMIT);
        assert_eq!(limit_for("a.log"), TEXT_LIMIT);
    }

    #[test]
    fn text_is_shown_and_a_long_file_says_it_was_cut() {
        let Content::Text { text, cut } = decode("a.txt", "zażółć\n".as_bytes(), true) else {
            panic!("not text");
        };
        assert_eq!(text, "zażółć\n");
        assert!(cut);
    }

    #[test]
    fn a_file_with_zero_bytes_is_binary() {
        assert!(matches!(
            decode("a.bin", b"MZ\0\0\x03", false),
            Content::Binary
        ));
    }

    #[test]
    fn a_small_picture_is_decoded() {
        let content = decode("a.png", &png_bytes(3, 2), false);

        assert!(matches!(content, Content::Picture(_)), "{content:?}");
    }

    #[test]
    fn a_picture_cut_at_the_limit_is_too_large() {
        assert!(matches!(
            decode("a.png", &png_bytes(3, 2), true),
            Content::TooLarge
        ));
    }

    #[test]
    fn a_broken_picture_gives_a_reason_instead_of_a_crash() {
        assert!(matches!(
            decode("a.png", b"not a picture", false),
            Content::Broken(_)
        ));
    }

    #[test]
    fn a_picture_that_would_need_too_much_memory_is_refused() {
        // A few kilobytes of PNG that decode to 16 MB, against a limit of 1 MB.
        let small_file = png_bytes(2_000, 2_000);
        assert!(small_file.len() < 100_000);

        assert!(picture(&small_file, 1024 * 1024).is_err());
        assert!(picture(&small_file, DECODED_LIMIT).is_ok());
    }
}
