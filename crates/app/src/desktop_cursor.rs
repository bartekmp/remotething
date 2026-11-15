//! The remote pointer, drawn over the picture where the local pointer is.
//!
//! The window toolkit cannot show a pointer from a picture, so the local
//! pointer is hidden while it is over the desktop and the server's pointer is
//! drawn in its place.

use iced::widget::{image, pin, stack};
use iced::{Element, Length, Point, Size};
use remotething_rdp::Cursor;

use crate::desktop::Message;

/// A pointer picture ready to draw.
#[derive(Clone)]
pub(crate) struct Shape {
    handle: image::Handle,
    size: (u16, u16),
    hotspot: (u16, u16),
}

/// How the pointer looks over the desktop.
#[derive(Clone, Default)]
pub(crate) enum Look {
    /// The local system's arrow, until the server says otherwise.
    #[default]
    Local,
    Hidden,
    Shape(Shape),
}

impl Look {
    pub(crate) fn from_server(cursor: &Cursor) -> Look {
        match cursor {
            Cursor::Default => Look::Local,
            Cursor::Hidden => Look::Hidden,
            Cursor::Shape(shape) => Look::Shape(Shape {
                handle: image::Handle::from_rgba(
                    u32::from(shape.width),
                    u32::from(shape.height),
                    shape.rgba.clone(),
                ),
                size: (shape.width, shape.height),
                hotspot: (shape.hotspot_x, shape.hotspot_y),
            }),
        }
    }

    /// Whether the local pointer has to be hidden over the desktop.
    pub(crate) fn hides_local(&self) -> bool {
        !matches!(self, Look::Local)
    }
}

/// `picture` with the pointer drawn over it at `pointer`, when the pointer is over it.
pub(crate) fn with_pointer<'a>(
    picture: Element<'a, Message>,
    look: &Look,
    pointer: (u16, u16),
    over: bool,
    view: Size,
    remote: (u32, u32),
) -> Element<'a, Message> {
    let Look::Shape(shape) = look else {
        return picture;
    };
    if !over {
        return picture;
    }
    let (at, size) = placement(view, remote, pointer, shape.size, shape.hotspot);
    let drawn = image(shape.handle.clone())
        .width(Length::Fixed(size.width))
        .height(Length::Fixed(size.height));
    stack![picture, pin(drawn).position(at)].into()
}

/// Where the pointer picture goes in the view, and how big it is there.
///
/// The picture of the desktop is scaled to fit the view and centred, so the
/// pointer picture is scaled the same way and shifted by its hotspot.
pub(crate) fn placement(
    view: Size,
    remote: (u32, u32),
    pointer: (u16, u16),
    size: (u16, u16),
    hotspot: (u16, u16),
) -> (Point, Size) {
    let (width, height) = (remote.0.max(1) as f32, remote.1.max(1) as f32);
    let scale = (view.width / width).min(view.height / height).max(0.0);
    let left = (view.width - width * scale) / 2.0;
    let top = (view.height - height * scale) / 2.0;
    let at = Point::new(
        left + (f32::from(pointer.0) - f32::from(hotspot.0)) * scale,
        top + (f32::from(pointer.1) - f32::from(hotspot.1)) * scale,
    );
    let shown = Size::new(f32::from(size.0) * scale, f32::from(size.1) * scale);
    (at, shown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use remotething_rdp::CursorShape;

    #[test]
    fn the_hotspot_sits_on_the_pointer_position() {
        let (at, size) = placement(
            Size::new(1000.0, 800.0),
            (1000, 800),
            (100, 50),
            (32, 32),
            (4, 6),
        );

        assert_eq!((at.x, at.y), (96.0, 44.0));
        assert_eq!((size.width, size.height), (32.0, 32.0));
    }

    #[test]
    fn a_scaled_picture_scales_the_pointer_too() {
        // 2000 x 1600 remote pixels shown in 1000 x 800: half size.
        let (at, size) = placement(
            Size::new(1000.0, 800.0),
            (2000, 1600),
            (200, 100),
            (32, 32),
            (0, 0),
        );

        assert_eq!((at.x, at.y), (100.0, 50.0));
        assert_eq!((size.width, size.height), (16.0, 16.0));
    }

    #[test]
    fn bars_around_the_picture_shift_the_pointer() {
        // A 1000 x 800 picture in a 1200 x 800 view has 100 pixels of bar on each side.
        let (at, _) = placement(
            Size::new(1200.0, 800.0),
            (1000, 800),
            (0, 0),
            (16, 16),
            (0, 0),
        );

        assert_eq!(at.x, 100.0);
    }

    #[test]
    fn only_the_local_arrow_leaves_the_local_pointer_showing() {
        let shape = Cursor::Shape(CursorShape {
            width: 1,
            height: 1,
            hotspot_x: 0,
            hotspot_y: 0,
            rgba: vec![0; 4],
        });

        assert!(!Look::from_server(&Cursor::Default).hides_local());
        assert!(Look::from_server(&Cursor::Hidden).hides_local());
        assert!(Look::from_server(&shape).hides_local());
    }
}
