//! Pictures as PNG files.

/// Encodes RGBA pixels (4 bytes each, row by row from the top) as a PNG file.
///
/// Fails when the pixels do not fit the size or the size is zero.
pub fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("the picture is empty".to_string());
    }
    let expected = u64::from(width) * u64::from(height) * 4;
    if rgba.len() as u64 != expected {
        return Err("the pixels do not fit the size of the picture".to_string());
    }
    let mut output = Vec::new();
    let mut encoder = png::Encoder::new(&mut output, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(rgba)
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8]) -> (png::OutputInfo, Vec<u8>) {
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buffer).unwrap();
        buffer.truncate(info.buffer_size());
        (info, buffer)
    }

    #[test]
    fn a_picture_comes_back_the_same_from_its_png() {
        let pixels = vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 9, 8, 7, 6];

        let bytes = encode(2, 2, &pixels).unwrap();

        let (info, decoded) = decode(&bytes);
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(decoded, pixels);
    }

    #[test]
    fn the_file_starts_with_the_png_signature() {
        let bytes = encode(1, 1, &[0, 0, 0, 255]).unwrap();

        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
    }

    #[test]
    fn pixels_that_do_not_fit_are_refused() {
        assert!(encode(2, 2, &[0; 12]).is_err());
    }

    #[test]
    fn an_empty_picture_is_refused() {
        assert!(encode(0, 5, &[]).is_err());
    }
}
