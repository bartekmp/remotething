//! The mouse pointer as the server wants it drawn.

use ironrdp_graphics::pointer::DecodedPointer;

/// The picture of a pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorShape {
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// The pixel of the picture that sits on the pointer position, from the left.
    pub hotspot_x: u16,
    /// The pixel that sits on the pointer position, from the top.
    pub hotspot_y: u16,
    /// Red, green, blue and alpha of every pixel, row by row; alpha is not premultiplied.
    pub rgba: Vec<u8>,
}

/// What the server wants the pointer to look like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cursor {
    /// The ordinary arrow of the client's system.
    Default,
    /// No pointer at all.
    Hidden,
    /// A picture sent by the server.
    Shape(CursorShape),
}

impl Cursor {
    /// The shape in `pointer`, or `None` when its pixels do not fit its size.
    pub(crate) fn from_decoded(pointer: &DecodedPointer) -> Option<Cursor> {
        let expected = usize::from(pointer.width) * usize::from(pointer.height) * 4;
        if pointer.bitmap_data.len() != expected || expected == 0 {
            return None;
        }
        Some(Cursor::Shape(CursorShape {
            width: pointer.width,
            height: pointer.height,
            hotspot_x: pointer.hotspot_x,
            hotspot_y: pointer.hotspot_y,
            rgba: pointer.bitmap_data.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(width: u16, height: u16, bytes: usize) -> DecodedPointer {
        DecodedPointer {
            width,
            height,
            hotspot_x: 1,
            hotspot_y: 2,
            bitmap_data: vec![255; bytes],
        }
    }

    #[test]
    fn a_well_formed_pointer_keeps_its_size_and_hotspot() {
        let cursor = Cursor::from_decoded(&pointer(2, 3, 24)).unwrap();

        let Cursor::Shape(shape) = cursor else {
            panic!("expected a shape");
        };
        assert_eq!((shape.width, shape.height), (2, 3));
        assert_eq!((shape.hotspot_x, shape.hotspot_y), (1, 2));
        assert_eq!(shape.rgba.len(), 24);
    }

    #[test]
    fn pixels_that_do_not_fit_the_size_are_refused() {
        assert_eq!(Cursor::from_decoded(&pointer(2, 3, 20)), None);
    }

    #[test]
    fn an_empty_pointer_is_refused() {
        assert_eq!(Cursor::from_decoded(&pointer(0, 0, 0)), None);
    }
}
