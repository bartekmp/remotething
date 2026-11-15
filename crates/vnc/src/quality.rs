//! What each quality preset asks of the server.

use vnc::PixelFormat;

/// How much to trade picture quality for bandwidth.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Quality {
    /// The best picture, for a local network.
    Lan,
    /// The default trade-off.
    #[default]
    Balanced,
    /// The least traffic, for slow links.
    LowBandwidth,
}

/// How pixels travel on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Depth {
    /// Four bytes per pixel: red, green, blue and a spare byte.
    Bits32,
    /// Two bytes per pixel: five bits of red, six of green, five of blue.
    Bits16,
}

impl Quality {
    pub(crate) fn depth(self) -> Depth {
        match self {
            Quality::Lan | Quality::Balanced => Depth::Bits32,
            Quality::LowBandwidth => Depth::Bits16,
        }
    }

    /// Whether the server's pointer picture is asked for. Pictures of the
    /// pointer only come in 32 bits, so the low-bandwidth preset goes without.
    pub(crate) fn shows_pointer(self) -> bool {
        self.depth() == Depth::Bits32
    }
}

impl Depth {
    /// The pixel format to ask the server for.
    pub(crate) fn pixel_format(self) -> PixelFormat {
        match self {
            Depth::Bits32 => PixelFormat::rgba(),
            Depth::Bits16 => {
                let mut format = PixelFormat::default();
                format.bits_per_pixel = 16;
                format.depth = 16;
                format.red_max = 31;
                format.green_max = 63;
                format.blue_max = 31;
                format.red_shift = 11;
                format.green_shift = 5;
                format.blue_shift = 0;
                format
            }
        }
    }

    /// A short name for the depth, for showing to the user.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Depth::Bits32 => "32 bit",
            Depth::Bits16 => "16 bit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_low_bandwidth_preset_uses_16_bits_and_gives_up_the_pointer_picture() {
        assert_eq!(Quality::Lan.depth(), Depth::Bits32);
        assert_eq!(Quality::Balanced.depth(), Depth::Bits32);
        assert_eq!(Quality::LowBandwidth.depth(), Depth::Bits16);
        assert!(Quality::Balanced.shows_pointer());
        assert!(!Quality::LowBandwidth.shows_pointer());
    }

    #[test]
    fn the_formats_have_the_sizes_the_depths_say() {
        assert_eq!(Depth::Bits32.pixel_format().bits_per_pixel, 32);
        assert_eq!(Depth::Bits16.pixel_format().bits_per_pixel, 16);
    }

    #[test]
    fn the_16_bit_format_packs_5_6_5() {
        let format = Depth::Bits16.pixel_format();

        assert_eq!(
            (format.red_max, format.green_max, format.blue_max),
            (31, 63, 31)
        );
        assert_eq!(
            (format.red_shift, format.green_shift, format.blue_shift),
            (11, 5, 0)
        );
    }
}
