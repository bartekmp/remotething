//! Pictures on the system clipboard. The window toolkit only handles text.

use std::borrow::Cow;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Mutex, OnceLock};

use remotething_rdp::Picture;

/// One clipboard for the whole run: on X11 a picture put there is only served
/// while the object that put it there is alive.
static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();

fn with_clipboard<T>(action: impl FnOnce(&mut arboard::Clipboard) -> Option<T>) -> Option<T> {
    let cell = CLIPBOARD.get_or_init(|| Mutex::new(arboard::Clipboard::new().ok()));
    let mut guard = cell.lock().ok()?;
    action(guard.as_mut()?)
}

/// The picture on the clipboard, if it holds one. This can block for a moment.
pub(crate) fn read() -> Option<Picture> {
    with_clipboard(|clipboard| {
        let image = clipboard.get_image().ok()?;
        Some(Picture {
            width: u32::try_from(image.width).ok()?,
            height: u32::try_from(image.height).ok()?,
            rgba: image.bytes.into_owned(),
        })
    })
}

/// Puts a picture on the clipboard. True when it worked.
pub(crate) fn write(picture: &Picture) -> bool {
    with_clipboard(|clipboard| {
        let image = arboard::ImageData {
            width: picture.width as usize,
            height: picture.height as usize,
            bytes: Cow::Borrowed(&picture.rgba),
        };
        clipboard.set_image(image).ok()
    })
    .is_some()
}

/// A number that differs between pictures, to tell whether the clipboard changed.
pub(crate) fn fingerprint(picture: &Picture) -> u64 {
    let mut hasher = DefaultHasher::new();
    picture.width.hash(&mut hasher);
    picture.height.hash(&mut hasher);
    picture.rgba.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(pixel: u8) -> Picture {
        Picture {
            width: 1,
            height: 1,
            rgba: vec![pixel, 0, 0, 255],
        }
    }

    #[test]
    fn the_same_picture_has_the_same_fingerprint() {
        assert_eq!(fingerprint(&picture(1)), fingerprint(&picture(1)));
    }

    #[test]
    fn a_changed_pixel_changes_the_fingerprint() {
        assert_ne!(fingerprint(&picture(1)), fingerprint(&picture(2)));
    }

    #[test]
    fn a_different_size_changes_the_fingerprint() {
        let wide = Picture {
            width: 2,
            height: 1,
            rgba: vec![0; 8],
        };
        let tall = Picture {
            width: 1,
            height: 2,
            rgba: vec![0; 8],
        };

        assert_ne!(fingerprint(&wide), fingerprint(&tall));
    }
}
