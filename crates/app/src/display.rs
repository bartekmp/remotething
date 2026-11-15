//! The window's scale factor: how many screen pixels one logical pixel takes.
//!
//! The window reports it when it opens and when it moves to another screen.
//! Remote desktops read it to ask for a picture in real screen pixels.

use std::sync::atomic::{AtomicU32, Ordering};

/// The factor in thousandths, so it fits an atomic integer.
static SCALE_MILLI: AtomicU32 = AtomicU32::new(1000);

pub(crate) fn set_scale(scale: f32) {
    if scale.is_finite() && scale > 0.0 {
        SCALE_MILLI.store((scale * 1000.0).round() as u32, Ordering::Relaxed);
    }
}

pub(crate) fn scale() -> f32 {
    SCALE_MILLI.load(Ordering::Relaxed) as f32 / 1000.0
}

/// The factor as the percentage RDP takes, kept to what the protocol allows.
pub(crate) fn scale_percent() -> u32 {
    ((scale() * 100.0).round() as u32).clamp(100, 500)
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test, because the factor is shared by the whole process.
    #[test]
    fn the_scale_is_kept_and_turned_into_a_percentage() {
        set_scale(1.5);
        assert_eq!(scale(), 1.5);
        assert_eq!(scale_percent(), 150);

        set_scale(0.0);
        set_scale(f32::NAN);
        assert_eq!(scale(), 1.5);

        set_scale(0.5);
        assert_eq!(scale_percent(), 100);
        set_scale(9.0);
        assert_eq!(scale_percent(), 500);

        set_scale(1.0);
    }
}
