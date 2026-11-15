//! The app's logo: the window icon and the picture in the About block.

use std::sync::OnceLock;

use iced::widget::{Image, image};
use iced::window;

/// The logo as raw RGBA pixels, `SIZE` by `SIZE`. Made from `assets/logo/remotething.svg`;
/// see CONTRIBUTING.md for how to renew it.
const RGBA: &[u8] = include_bytes!("../assets/icon-128.rgba");
const SIZE: u32 = 128;

/// The icon for the window and the taskbar.
pub fn window_icon() -> Option<window::Icon> {
    window::icon::from_rgba(RGBA.to_vec(), SIZE, SIZE).ok()
}

/// The logo as a widget, `size` pixels wide and high.
pub fn logo(size: f32) -> Image {
    // One handle for the whole run, so the picture is uploaded once.
    static HANDLE: OnceLock<image::Handle> = OnceLock::new();
    let handle = HANDLE.get_or_init(|| image::Handle::from_rgba(SIZE, SIZE, RGBA.to_vec()));
    image(handle.clone()).width(size).height(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_pixels_fit_the_stated_size() {
        assert_eq!(RGBA.len(), (SIZE * SIZE * 4) as usize);
    }

    #[test]
    fn the_logo_makes_a_window_icon() {
        assert!(window_icon().is_some());
    }

    #[test]
    fn the_corners_of_the_tile_are_transparent() {
        // The first pixel is the very corner of the rounded tile.
        assert_eq!(RGBA[3], 0);
    }
}
