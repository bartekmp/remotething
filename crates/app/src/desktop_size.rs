//! Which size to ask the remote desktop for, given the room the tab has.

use iced::Size;

/// The smallest and largest desktop the protocol allows, in pixels.
const MIN: f32 = 200.0;
const MAX: f32 = 8192.0;
/// A difference of this many pixels is not worth a new request.
const TOLERANCE: u32 = 2;

/// The size to ask for, or `None` when the desktop already fits `area`.
///
/// The width is rounded down to an even number, which the protocol requires.
pub(crate) fn wanted(area: Size, current: (u32, u32)) -> Option<(u16, u16)> {
    if area.width < MIN || area.height < MIN {
        return None;
    }
    let width = (area.width.min(MAX) as u32) & !1;
    let height = area.height.min(MAX) as u32;
    let close = width.abs_diff(current.0) <= TOLERANCE && height.abs_diff(current.1) <= TOLERANCE;
    if close {
        return None;
    }
    Some((u16::try_from(width).ok()?, u16::try_from(height).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_grows_to_the_room() {
        assert_eq!(
            wanted(Size::new(1500.0, 900.0), (1280, 800)),
            Some((1500, 900))
        );
    }

    #[test]
    fn an_odd_width_is_made_even() {
        assert_eq!(
            wanted(Size::new(1501.0, 900.0), (1280, 800)),
            Some((1500, 900))
        );
    }

    #[test]
    fn a_desktop_that_already_fits_is_left_alone() {
        assert_eq!(wanted(Size::new(1281.0, 799.0), (1280, 800)), None);
    }

    #[test]
    fn a_tab_that_is_too_small_is_ignored() {
        assert_eq!(wanted(Size::new(150.0, 900.0), (1280, 800)), None);
        assert_eq!(wanted(Size::new(900.0, 100.0), (1280, 800)), None);
    }

    #[test]
    fn a_huge_area_is_capped() {
        assert_eq!(
            wanted(Size::new(20000.0, 9000.0), (1280, 800)),
            Some((8192, 8192))
        );
    }
}
