//! The RDP quality presets as the user sees them.

use std::fmt;

use remotething_core::Quality;

use crate::i18n::fl;

/// The name of a preset.
pub(crate) fn label(quality: Quality) -> String {
    match quality {
        Quality::Lan => fl!("quality-lan"),
        Quality::Balanced => fl!("quality-balanced"),
        Quality::LowBandwidth => fl!("quality-low-bandwidth"),
    }
}

/// A preset in a drop-down, or "inherit" when none is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityChoice(pub Option<Quality>);

pub(crate) const CHOICES: [QualityChoice; 4] = [
    QualityChoice(None),
    QualityChoice(Some(Quality::Lan)),
    QualityChoice(Some(Quality::Balanced)),
    QualityChoice(Some(Quality::LowBandwidth)),
];

impl fmt::Display for QualityChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(quality) => formatter.write_str(&label(quality)),
            None => formatter.write_str(&fl!("properties-inherit")),
        }
    }
}

/// The preset in the form the RDP crate takes.
pub(crate) fn for_session(quality: Quality) -> remotething_rdp::Quality {
    match quality {
        Quality::Lan => remotething_rdp::Quality::Lan,
        Quality::Balanced => remotething_rdp::Quality::Balanced,
        Quality::LowBandwidth => remotething_rdp::Quality::LowBandwidth,
    }
}

/// The preset in the form the VNC crate takes.
pub(crate) fn for_vnc(quality: Quality) -> remotething_vnc::Quality {
    match quality {
        Quality::Lan => remotething_vnc::Quality::Lan,
        Quality::Balanced => remotething_vnc::Quality::Balanced,
        Quality::LowBandwidth => remotething_vnc::Quality::LowBandwidth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_maps_to_its_own() {
        assert_eq!(for_session(Quality::Lan), remotething_rdp::Quality::Lan);
        assert_eq!(
            for_session(Quality::Balanced),
            remotething_rdp::Quality::Balanced
        );
        assert_eq!(
            for_session(Quality::LowBandwidth),
            remotething_rdp::Quality::LowBandwidth
        );
    }

    #[test]
    fn every_preset_maps_to_its_own_for_vnc() {
        assert_eq!(for_vnc(Quality::Lan), remotething_vnc::Quality::Lan);
        assert_eq!(
            for_vnc(Quality::Balanced),
            remotething_vnc::Quality::Balanced
        );
        assert_eq!(
            for_vnc(Quality::LowBandwidth),
            remotething_vnc::Quality::LowBandwidth
        );
    }

    #[test]
    fn the_drop_down_starts_with_inherit() {
        assert_eq!(CHOICES[0], QualityChoice(None));
        assert_eq!(CHOICES.len(), 4);
    }
}
