//! The desktop picture, kept up to date from the server's rectangles.

use crate::quality::Depth;

/// 16-bit pixels, two bytes each with the low byte first, as RGBA.
pub(crate) fn rgb565_to_rgba(data: &[u8]) -> Vec<u8> {
    // A channel's top bits are repeated into its low bits, so full intensity stays full.
    let widen5 = |value: u16| ((value << 3) | (value >> 2)) as u8;
    let widen6 = |value: u16| ((value << 2) | (value >> 4)) as u8;
    data.as_chunks::<2>()
        .0
        .iter()
        .flat_map(|pair| {
            let pixel = u16::from_le_bytes(*pair);
            [
                widen5(pixel >> 11 & 0x1f),
                widen6(pixel >> 5 & 0x3f),
                widen5(pixel & 0x1f),
                255,
            ]
        })
        .collect()
}

/// A rectangle of the desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Area {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) width: u16,
    pub(crate) height: u16,
}

/// The whole picture as RGBA, 4 bytes per pixel, always opaque.
pub(crate) struct Framebuffer {
    width: u16,
    height: u16,
    rgba: Vec<u8>,
}

impl Framebuffer {
    /// A black picture.
    pub(crate) fn new(width: u16, height: u16) -> Self {
        let mut rgba = vec![0; usize::from(width) * usize::from(height) * 4];
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }
        Self {
            width,
            height,
            rgba,
        }
    }

    pub(crate) fn width(&self) -> u16 {
        self.width
    }

    pub(crate) fn height(&self) -> u16 {
        self.height
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.rgba
    }

    /// Whether `area` lies inside the picture.
    fn contains(&self, area: Area) -> bool {
        u32::from(area.x) + u32::from(area.width) <= u32::from(self.width)
            && u32::from(area.y) + u32::from(area.height) <= u32::from(self.height)
    }

    /// Paints RGBA `data` into `area`. Data of the wrong length or a rectangle
    /// outside the picture is ignored; the server is not trusted to be right.
    pub(crate) fn paint(&mut self, area: Area, data: &[u8]) {
        let row = usize::from(area.width) * 4;
        if !self.contains(area) || data.len() != row * usize::from(area.height) {
            return;
        }
        for line in 0..usize::from(area.height) {
            let to = self.offset(area.x, area.y + line as u16);
            let from = line * row;
            self.rgba[to..to + row].copy_from_slice(&data[from..from + row]);
            // The server does not send alpha.
            for pixel in self.rgba[to..to + row].as_chunks_mut::<4>().0 {
                pixel[3] = 255;
            }
        }
    }

    /// Paints pixels as they came from the server in `depth` into `area`.
    pub(crate) fn paint_depth(&mut self, area: Area, data: &[u8], depth: Depth) {
        match depth {
            Depth::Bits32 => self.paint(area, data),
            Depth::Bits16 => self.paint(area, &rgb565_to_rgba(data)),
        }
    }

    /// Copies the pixels of `source` to the place of `target`, which has the same size.
    pub(crate) fn copy(&mut self, target: Area, source: Area) {
        if target.width != source.width
            || target.height != source.height
            || !self.contains(target)
            || !self.contains(source)
        {
            return;
        }
        let row = usize::from(source.width) * 4;
        // Going through a copy keeps overlapping areas right.
        let mut taken = Vec::with_capacity(row * usize::from(source.height));
        for line in 0..source.height {
            let from = self.offset(source.x, source.y + line);
            taken.extend_from_slice(&self.rgba[from..from + row]);
        }
        for line in 0..usize::from(target.height) {
            let to = self.offset(target.x, target.y + line as u16);
            self.rgba[to..to + row].copy_from_slice(&taken[line * row..(line + 1) * row]);
        }
    }

    fn offset(&self, x: u16, y: u16) -> usize {
        (usize::from(y) * usize::from(self.width) + usize::from(x)) * 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: u16, y: u16, width: u16, height: u16) -> Area {
        Area {
            x,
            y,
            width,
            height,
        }
    }

    fn pixel(buffer: &Framebuffer, x: u16, y: u16) -> [u8; 4] {
        let at = buffer.offset(x, y);
        buffer.rgba[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn sixteen_bit_pixels_are_widened_to_full_range() {
        // White, pure red, pure green, pure blue, black.
        let data = [0xff, 0xff, 0x00, 0xf8, 0xe0, 0x07, 0x1f, 0x00, 0x00, 0x00];

        let rgba = rgb565_to_rgba(&data);

        assert_eq!(
            rgba,
            [
                255, 255, 255, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 255
            ]
        );
    }

    #[test]
    fn painting_in_16_bits_takes_two_bytes_per_pixel() {
        let mut buffer = Framebuffer::new(2, 1);

        buffer.paint_depth(area(0, 0, 2, 1), &[0x00, 0xf8, 0x1f, 0x00], Depth::Bits16);

        assert_eq!(pixel(&buffer, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&buffer, 1, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn a_new_picture_is_opaque_black() {
        let buffer = Framebuffer::new(2, 2);

        assert_eq!(pixel(&buffer, 1, 1), [0, 0, 0, 255]);
    }

    #[test]
    fn painted_pixels_land_where_the_area_says_and_are_made_opaque() {
        let mut buffer = Framebuffer::new(4, 4);

        buffer.paint(area(1, 2, 2, 1), &[1, 2, 3, 0, 4, 5, 6, 0]);

        assert_eq!(pixel(&buffer, 1, 2), [1, 2, 3, 255]);
        assert_eq!(pixel(&buffer, 2, 2), [4, 5, 6, 255]);
        assert_eq!(pixel(&buffer, 0, 2), [0, 0, 0, 255]);
    }

    #[test]
    fn a_rectangle_outside_the_picture_is_ignored() {
        let mut buffer = Framebuffer::new(4, 4);

        buffer.paint(area(3, 3, 2, 2), &[9; 16]);

        assert_eq!(pixel(&buffer, 3, 3), [0, 0, 0, 255]);
    }

    #[test]
    fn data_of_the_wrong_length_is_ignored() {
        let mut buffer = Framebuffer::new(4, 4);

        buffer.paint(area(0, 0, 2, 2), &[9; 12]);

        assert_eq!(pixel(&buffer, 0, 0), [0, 0, 0, 255]);
    }

    #[test]
    fn a_copy_moves_pixels_and_handles_overlap() {
        let mut buffer = Framebuffer::new(4, 1);
        buffer.paint(area(0, 0, 3, 1), &[1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);

        buffer.copy(area(1, 0, 3, 1), area(0, 0, 3, 1));

        let reds: Vec<u8> = (0..4).map(|x| pixel(&buffer, x, 0)[0]).collect();
        assert_eq!(reds, [1, 1, 2, 3]);
    }

    #[test]
    fn a_copy_between_areas_of_different_size_is_ignored() {
        let mut buffer = Framebuffer::new(4, 4);
        buffer.paint(area(0, 0, 1, 1), &[7, 7, 7, 0]);

        buffer.copy(area(2, 2, 2, 2), area(0, 0, 1, 1));

        assert_eq!(pixel(&buffer, 2, 2), [0, 0, 0, 255]);
    }
}
