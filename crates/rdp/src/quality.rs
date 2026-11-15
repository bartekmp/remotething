//! What each quality preset asks of the server.

use ironrdp_connector::BitmapConfig;
use ironrdp_pdu::gcc::ConnectionType;
use ironrdp_pdu::rdp::capability_sets::client_codecs_capabilities;
use ironrdp_pdu::rdp::client_info::PerformanceFlags;

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

/// The connection settings behind a preset.
pub(crate) struct Profile {
    pub(crate) color_depth: u32,
    pub(crate) performance: PerformanceFlags,
    pub(crate) connection_type: ConnectionType,
}

impl Quality {
    pub(crate) fn profile(self) -> Profile {
        match self {
            Quality::Lan => Profile {
                color_depth: 32,
                performance: PerformanceFlags::ENABLE_FONT_SMOOTHING
                    | PerformanceFlags::ENABLE_DESKTOP_COMPOSITION,
                connection_type: ConnectionType::Lan,
            },
            Quality::Balanced => Profile {
                color_depth: 32,
                performance: PerformanceFlags::DISABLE_WALLPAPER
                    | PerformanceFlags::DISABLE_FULLWINDOWDRAG
                    | PerformanceFlags::DISABLE_MENUANIMATIONS
                    | PerformanceFlags::ENABLE_FONT_SMOOTHING,
                connection_type: ConnectionType::BroadbandHigh,
            },
            Quality::LowBandwidth => Profile {
                color_depth: 16,
                performance: PerformanceFlags::DISABLE_WALLPAPER
                    | PerformanceFlags::DISABLE_FULLWINDOWDRAG
                    | PerformanceFlags::DISABLE_MENUANIMATIONS
                    | PerformanceFlags::DISABLE_THEMING
                    | PerformanceFlags::DISABLE_CURSOR_SHADOW
                    | PerformanceFlags::DISABLE_CURSORSETTINGS,
                connection_type: ConnectionType::BroadbandLow,
            },
        }
    }
}

/// The picture codecs the client offers. RemoteFX compresses far better than classic bitmaps.
pub(crate) fn bitmap_config(color_depth: u32) -> Option<BitmapConfig> {
    let codecs = client_codecs_capabilities(&[]).ok()?;
    Some(BitmapConfig {
        lossy_compression: true,
        color_depth,
        codecs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_balanced() {
        assert_eq!(Quality::default(), Quality::Balanced);
    }

    #[test]
    fn only_the_low_bandwidth_preset_lowers_the_colour_depth() {
        assert_eq!(Quality::Lan.profile().color_depth, 32);
        assert_eq!(Quality::Balanced.profile().color_depth, 32);
        assert_eq!(Quality::LowBandwidth.profile().color_depth, 16);
    }

    #[test]
    fn the_wallpaper_is_only_sent_on_the_lan_preset() {
        let wallpaper = PerformanceFlags::DISABLE_WALLPAPER;
        assert!(!Quality::Lan.profile().performance.contains(wallpaper));
        assert!(Quality::Balanced.profile().performance.contains(wallpaper));
        assert!(
            Quality::LowBandwidth
                .profile()
                .performance
                .contains(wallpaper)
        );
    }

    #[test]
    fn the_codec_offer_is_not_empty() {
        let config = bitmap_config(32).expect("the default codecs are valid");

        assert!(config.lossy_compression);
        assert_eq!(config.color_depth, 32);
    }
}
