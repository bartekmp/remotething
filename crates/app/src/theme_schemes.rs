//! The named colour schemes of the app theme. Each is a few colours; `theme.rs` works out the
//! rest (panels, borders, hover tints) from them.

use iced::{Color, color};

use crate::preferences::ThemeChoice;

/// The colours that define a scheme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scheme {
    pub name: &'static str,
    pub dark: bool,
    /// Behind the panels.
    pub background: Color,
    pub text: Color,
    /// Buttons, links and the selected row.
    pub accent: Color,
    pub danger: Color,
}

const fn scheme(
    name: &'static str,
    dark: bool,
    background: Color,
    text: Color,
    accent: Color,
    danger: Color,
) -> Scheme {
    Scheme {
        name,
        dark,
        background,
        text,
        accent,
        danger,
    }
}

const DRACULA_DARK: Scheme = scheme(
    "Dracula Dark",
    true,
    color!(0x282A36),
    color!(0xF8F8F2),
    color!(0xBD93F9),
    color!(0xFF6E6E),
);
/// Alucard, the light variant of Dracula.
const DRACULA_LIGHT: Scheme = scheme(
    "Dracula Light",
    false,
    color!(0xFFFBEB),
    color!(0x1F1F1F),
    color!(0x644AC9),
    color!(0xB52A1C),
);
const CATPPUCCIN_MOCHA: Scheme = scheme(
    "Catppuccin Mocha",
    true,
    color!(0x1E1E2E),
    color!(0xCDD6F4),
    color!(0x89B4FA),
    color!(0xF38BA8),
);
const CATPPUCCIN_LATTE: Scheme = scheme(
    "Catppuccin Latte",
    false,
    color!(0xEFF1F5),
    color!(0x4C4F69),
    color!(0x1E66F5),
    color!(0xD20F39),
);
const GRUVBOX_DARK: Scheme = scheme(
    "Gruvbox Dark",
    true,
    color!(0x282828),
    color!(0xEBDBB2),
    color!(0x83A598),
    color!(0xFB4934),
);
const GRUVBOX_LIGHT: Scheme = scheme(
    "Gruvbox Light",
    false,
    color!(0xFBF1C7),
    color!(0x3C3836),
    color!(0x076678),
    color!(0x9D0006),
);
const NORD_DARK: Scheme = scheme(
    "Nord Dark",
    true,
    color!(0x2E3440),
    color!(0xD8DEE9),
    color!(0x88C0D0),
    color!(0xD9737C),
);
const NORD_LIGHT: Scheme = scheme(
    "Nord Light",
    false,
    color!(0xECEFF4),
    color!(0x2E3440),
    color!(0x5E81AC),
    color!(0xA93F4B),
);
const SOLARIZED_DARK: Scheme = scheme(
    "Solarized Dark",
    true,
    color!(0x002B36),
    color!(0x93A1A1),
    color!(0x268BD2),
    color!(0xDC322F),
);
const SOLARIZED_LIGHT: Scheme = scheme(
    "Solarized Light",
    false,
    color!(0xFDF6E3),
    color!(0x4F666E),
    color!(0x268BD2),
    color!(0xC02623),
);
const MONOKAI_DARK: Scheme = scheme(
    "Monokai Dark",
    true,
    color!(0x272822),
    color!(0xF8F8F2),
    color!(0xA6E22E),
    color!(0xF92672),
);
const MONOKAI_LIGHT: Scheme = scheme(
    "Monokai Light",
    false,
    color!(0xFAF9F5),
    color!(0x272822),
    color!(0x5E8A00),
    color!(0xC4004E),
);

/// Every named scheme, in the order of the Theme list: each family dark, then light.
pub const ALL: [(ThemeChoice, Scheme); 12] = [
    (ThemeChoice::DraculaDark, DRACULA_DARK),
    (ThemeChoice::DraculaLight, DRACULA_LIGHT),
    (ThemeChoice::CatppuccinMocha, CATPPUCCIN_MOCHA),
    (ThemeChoice::CatppuccinLatte, CATPPUCCIN_LATTE),
    (ThemeChoice::GruvboxDark, GRUVBOX_DARK),
    (ThemeChoice::GruvboxLight, GRUVBOX_LIGHT),
    (ThemeChoice::NordDark, NORD_DARK),
    (ThemeChoice::NordLight, NORD_LIGHT),
    (ThemeChoice::SolarizedDark, SOLARIZED_DARK),
    (ThemeChoice::SolarizedLight, SOLARIZED_LIGHT),
    (ThemeChoice::MonokaiDark, MONOKAI_DARK),
    (ThemeChoice::MonokaiLight, MONOKAI_LIGHT),
];

/// The scheme behind a choice of the Theme list; `None` for the choices that are not one.
pub fn of(choice: ThemeChoice) -> Option<Scheme> {
    ALL.iter()
        .find(|(candidate, _)| *candidate == choice)
        .map(|(_, scheme)| *scheme)
}
