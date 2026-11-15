//! Windows device-independent bitmaps (the `CF_DIB` clipboard format) and RGBA pictures.

/// A picture of 4 bytes per pixel: red, green, blue and alpha, row by row from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The pixels.
    pub rgba: Vec<u8>,
}

/// The most pixels in a picture taken in or sent out; a clipboard picture can be a whole screen.
const MAX_PIXELS: u64 = 8192 * 8192;
/// The size of the BITMAPINFOHEADER, which every DIB starts with.
const HEADER: usize = 40;
const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;

fn le32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Reads a DIB of 24 or 32 bits per pixel, without compression.
///
/// Returns `None` for anything else, or when the data is too short for its header.
pub fn from_dib(dib: &[u8]) -> Option<Picture> {
    let header_size = le32(dib, 0)? as usize;
    if header_size < HEADER {
        return None;
    }
    let width = i32::from_le_bytes(dib.get(4..8)?.try_into().ok()?);
    let height = i32::from_le_bytes(dib.get(8..12)?.try_into().ok()?);
    let bits = u16::from_le_bytes(dib.get(14..16)?.try_into().ok()?);
    let compression = le32(dib, 16)?;
    if width <= 0 || height == 0 || !matches!(bits, 24 | 32) {
        return None;
    }
    if !matches!(compression, BI_RGB | BI_BITFIELDS) {
        return None;
    }
    // A negative height means the rows are stored top down.
    let top_down = height < 0;
    let (width, height) = (width as u32, height.unsigned_abs());
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return None;
    }
    // Three masks follow a plain 40-byte header that uses bit fields; the usual layouts are assumed.
    let masks = if compression == BI_BITFIELDS && header_size == HEADER {
        12
    } else {
        0
    };
    let start = header_size + masks;
    let bytes_per_pixel = usize::from(bits / 8);
    let stride = (width as usize * bytes_per_pixel).div_ceil(4) * 4;
    let rows = dib.get(start..start + stride * height as usize)?;

    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for line in 0..height as usize {
        let source = if top_down {
            line
        } else {
            height as usize - 1 - line
        };
        let row = &rows[source * stride..source * stride + width as usize * bytes_per_pixel];
        for pixel in row.chunks_exact(bytes_per_pixel) {
            let alpha = if bytes_per_pixel == 4 { pixel[3] } else { 255 };
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], alpha]);
        }
    }
    // Plain 32-bit DIBs leave the fourth byte unused, which then reads as fully transparent.
    if bits == 32 && rgba.chunks_exact(4).all(|pixel| pixel[3] == 0) {
        for pixel in rgba.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
    }
    Some(Picture {
        width,
        height,
        rgba,
    })
}

/// Writes a picture as a 32-bit DIB with its rows from the bottom up, as `CF_DIB` wants.
///
/// Returns `None` when the pixels do not fit the size or the picture is too big.
pub fn to_dib(picture: &Picture) -> Option<Vec<u8>> {
    let (width, height) = (picture.width, picture.height);
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || pixels > MAX_PIXELS || picture.rgba.len() as u64 != pixels * 4 {
        return None;
    }
    let mut dib = Vec::with_capacity(HEADER + picture.rgba.len());
    dib.extend_from_slice(&(HEADER as u32).to_le_bytes());
    dib.extend_from_slice(&i32::try_from(width).ok()?.to_le_bytes());
    dib.extend_from_slice(&i32::try_from(height).ok()?.to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&BI_RGB.to_le_bytes());
    // Image size, resolution and palette: none given.
    dib.extend_from_slice(&[0u8; 20]);
    for line in (0..height as usize).rev() {
        let row = &picture.rgba[line * width as usize * 4..(line + 1) * width as usize * 4];
        for pixel in row.chunks_exact(4) {
            dib.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    Some(dib)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture() -> Picture {
        // Two by two: red, green / blue, white-half-transparent.
        Picture {
            width: 2,
            height: 2,
            rgba: vec![
                255, 0, 0, 255, 0, 255, 0, 255, //
                0, 0, 255, 255, 255, 255, 255, 128,
            ],
        }
    }

    #[test]
    fn a_picture_survives_the_trip_through_a_dib() {
        let dib = to_dib(&picture()).unwrap();

        assert_eq!(from_dib(&dib), Some(picture()));
    }

    #[test]
    fn rows_are_stored_from_the_bottom() {
        let dib = to_dib(&picture()).unwrap();

        // After the header the first pixel is the bottom left one: blue, as BGRA.
        assert_eq!(&dib[HEADER..HEADER + 4], &[255, 0, 0, 255]);
    }

    #[test]
    fn a_24_bit_dib_with_padded_rows_is_read() {
        // Width 1: each row is 3 bytes of pixel and 1 of padding. Bottom row first.
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&24u16.to_le_bytes());
        dib.extend_from_slice(&[0u8; 24]);
        dib.extend_from_slice(&[0, 0, 255, 0]); // bottom: red (BGR)
        dib.extend_from_slice(&[255, 0, 0, 0]); // top: blue

        let read = from_dib(&dib).unwrap();

        assert_eq!(read.rgba, [0, 0, 255, 255, 255, 0, 0, 255]);
    }

    #[test]
    fn a_top_down_dib_keeps_its_row_order() {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&(-2i32).to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&[0u8; 24]);
        dib.extend_from_slice(&[0, 0, 255, 255]); // top: red
        dib.extend_from_slice(&[255, 0, 0, 255]); // bottom: blue

        let read = from_dib(&dib).unwrap();

        assert_eq!(read.rgba, [255, 0, 0, 255, 0, 0, 255, 255]);
    }

    #[test]
    fn an_unused_alpha_byte_does_not_make_the_picture_invisible() {
        let mut dib = to_dib(&Picture {
            width: 1,
            height: 1,
            rgba: vec![10, 20, 30, 255],
        })
        .unwrap();
        // The way a plain 32-bit DIB is written by Windows: alpha left at zero.
        let at = dib.len() - 1;
        dib[at] = 0;

        assert_eq!(from_dib(&dib).unwrap().rgba, [10, 20, 30, 255]);
    }

    #[test]
    fn data_that_is_cut_short_is_refused() {
        let dib = to_dib(&picture()).unwrap();

        assert_eq!(from_dib(&dib[..dib.len() - 1]), None);
        assert_eq!(from_dib(&[1, 2, 3]), None);
    }

    #[test]
    fn unsupported_depths_and_compression_are_refused() {
        let mut dib = to_dib(&picture()).unwrap();
        dib[14] = 8; // 8 bits per pixel
        assert_eq!(from_dib(&dib), None);

        let mut dib = to_dib(&picture()).unwrap();
        dib[16] = 1; // run-length compression
        assert_eq!(from_dib(&dib), None);
    }

    #[test]
    fn a_huge_size_is_refused_without_allocating_it() {
        let mut dib = to_dib(&picture()).unwrap();
        dib[4..8].copy_from_slice(&100_000i32.to_le_bytes());

        assert_eq!(from_dib(&dib), None);
    }

    #[test]
    fn pixels_that_do_not_fit_the_size_are_not_turned_into_a_dib() {
        let wrong = Picture {
            width: 2,
            height: 2,
            rgba: vec![0; 12],
        };

        assert_eq!(to_dib(&wrong), None);
    }
}
