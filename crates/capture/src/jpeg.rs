//! Pictures as JPEG, for the frames of a video.

use jpeg_encoder::{ColorType, Encoder};

/// How good the pictures are, from 1 to 100. Screens are text and edges, which
/// JPEG blurs at low values, so this is on the high side.
const QUALITY: u8 = 80;

/// Encodes RGBA pixels as a JPEG file. The alpha channel is left out.
///
/// Fails when the pixels do not fit the size, or the size is zero or too big for JPEG.
pub fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let (columns, rows) = match (u16::try_from(width), u16::try_from(height)) {
        (Ok(columns), Ok(rows)) if columns > 0 && rows > 0 => (columns, rows),
        _ => return Err("the picture is empty or too big for JPEG".to_string()),
    };
    if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
        return Err("the pixels do not fit the size of the picture".to_string());
    }
    let mut output = Vec::new();
    Encoder::new(&mut output, QUALITY)
        .encode(rgba, columns, rows, ColorType::Rgba)
        .map_err(|error| error.to_string())?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_becomes_a_jpeg_file() {
        let pixels = vec![200; 16 * 16 * 4];

        let bytes = encode(16, 16, &pixels).unwrap();

        // Starts with the start-of-image marker and ends with the end-of-image marker.
        assert_eq!(&bytes[..2], &[0xff, 0xd8]);
        assert_eq!(&bytes[bytes.len() - 2..], &[0xff, 0xd9]);
    }

    #[test]
    fn pixels_that_do_not_fit_are_refused() {
        assert!(encode(4, 4, &[0; 10]).is_err());
    }

    #[test]
    fn an_empty_or_huge_picture_is_refused() {
        assert!(encode(0, 4, &[]).is_err());
        assert!(encode(70_000, 1, &[]).is_err());
    }
}
