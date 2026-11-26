//! Terminal colour themes that come with the app.

use std::sync::OnceLock;

use crate::terminal_theme::{Rgb, Theme};

fn theme(name: &str, foreground: u32, background: u32, selection: u32, ansi: [u32; 16]) -> Theme {
    Theme {
        name: name.to_string(),
        foreground: Rgb::new(foreground),
        background: Rgb::new(background),
        selection: Some(Rgb::new(selection)),
        ansi: ansi.into_iter().map(Rgb::new).collect(),
    }
}

/// The themes that come with the app: each family dark, then light.
pub fn all() -> &'static [Theme] {
    static THEMES: OnceLock<Vec<Theme>> = OnceLock::new();
    THEMES.get_or_init(|| {
        vec![
            theme(
                "Dracula Dark",
                0xF8F8F2,
                0x282A36,
                0x44475A,
                [
                    0x21222C, 0xFF5555, 0x50FA7B, 0xF1FA8C, 0xBD93F9, 0xFF79C6, 0x8BE9FD, 0xF8F8F2,
                    0x6272A4, 0xFF6E6E, 0x69FF94, 0xFFFFA5, 0xD6ACFF, 0xFF92DF, 0xA4FFFF, 0xFFFFFF,
                ],
            ),
            // Alucard, the light variant of Dracula.
            theme(
                "Dracula Light",
                0x1F1F1F,
                0xFFFBEB,
                0xCFCFDE,
                [
                    0x1F1F1F, 0xCB3A2A, 0x14710A, 0x846E15, 0x644AC9, 0xA3144D, 0x036A96, 0xCFCFDE,
                    0x6C664B, 0xD74C3D, 0x198C0E, 0x9E8419, 0x7C62E0, 0xBF2461, 0x0586B8, 0xFFFBEB,
                ],
            ),
            theme(
                "Catppuccin Mocha",
                0xCDD6F4,
                0x1E1E2E,
                0x585B70,
                [
                    0x45475A, 0xF38BA8, 0xA6E3A1, 0xF9E2AF, 0x89B4FA, 0xF5C2E7, 0x94E2D5, 0xBAC2DE,
                    0x585B70, 0xF38BA8, 0xA6E3A1, 0xF9E2AF, 0x89B4FA, 0xF5C2E7, 0x94E2D5, 0xA6ADC8,
                ],
            ),
            theme(
                "Catppuccin Latte",
                0x4C4F69,
                0xEFF1F5,
                0xACB0BE,
                [
                    0x5C5F77, 0xD20F39, 0x40A02B, 0xDF8E1D, 0x1E66F5, 0xEA76CB, 0x179299, 0xACB0BE,
                    0x6C6F85, 0xD20F39, 0x40A02B, 0xDF8E1D, 0x1E66F5, 0xEA76CB, 0x179299, 0xBCC0CC,
                ],
            ),
            theme(
                "Gruvbox Dark",
                0xEBDBB2,
                0x282828,
                0x504945,
                [
                    0x282828, 0xCC241D, 0x98971A, 0xD79921, 0x458588, 0xB16286, 0x689D6A, 0xA89984,
                    0x928374, 0xFB4934, 0xB8BB26, 0xFABD2F, 0x83A598, 0xD3869B, 0x8EC07C, 0xEBDBB2,
                ],
            ),
            theme(
                "Gruvbox Light",
                0x3C3836,
                0xFBF1C7,
                0xD5C4A1,
                [
                    0x3C3836, 0xCC241D, 0x98971A, 0xD79921, 0x458588, 0xB16286, 0x689D6A, 0x7C6F64,
                    0x928374, 0x9D0006, 0x79740E, 0xB57614, 0x076678, 0x8F3F71, 0x427B58, 0x282828,
                ],
            ),
            theme(
                "Nord Dark",
                0xD8DEE9,
                0x2E3440,
                0x434C5E,
                [
                    0x3B4252, 0xBF616A, 0xA3BE8C, 0xEBCB8B, 0x81A1C1, 0xB48EAD, 0x88C0D0, 0xE5E9F0,
                    0x4C566A, 0xBF616A, 0xA3BE8C, 0xEBCB8B, 0x81A1C1, 0xB48EAD, 0x8FBCBB, 0xECEFF4,
                ],
            ),
            theme(
                "Nord Light",
                0x2E3440,
                0xECEFF4,
                0xD8DEE9,
                [
                    0x3B4252, 0xA93F4B, 0x5F7F48, 0x9A7B25, 0x5E81AC, 0x8F6A89, 0x3F8A9B, 0xD8DEE9,
                    0x4C566A, 0xBF616A, 0x6F9356, 0xB58E2C, 0x6D92BF, 0xA4799E, 0x4E9DAF, 0xECEFF4,
                ],
            ),
            theme(
                "Solarized Dark",
                0x839496,
                0x002B36,
                0x073642,
                [
                    0x073642, 0xDC322F, 0x859900, 0xB58900, 0x268BD2, 0xD33682, 0x2AA198, 0xEEE8D5,
                    0x002B36, 0xCB4B16, 0x586E75, 0x657B83, 0x839496, 0x6C71C4, 0x93A1A1, 0xFDF6E3,
                ],
            ),
            theme(
                "Solarized Light",
                0x657B83,
                0xFDF6E3,
                0xEEE8D5,
                [
                    0x073642, 0xDC322F, 0x859900, 0xB58900, 0x268BD2, 0xD33682, 0x2AA198, 0xEEE8D5,
                    0x002B36, 0xCB4B16, 0x586E75, 0x657B83, 0x839496, 0x6C71C4, 0x93A1A1, 0xFDF6E3,
                ],
            ),
            theme(
                "Monokai Dark",
                0xF8F8F2,
                0x272822,
                0x49483E,
                [
                    0x272822, 0xF92672, 0xA6E22E, 0xF4BF75, 0x66D9EF, 0xAE81FF, 0xA1EFE4, 0xF8F8F2,
                    0x75715E, 0xF92672, 0xA6E22E, 0xF4BF75, 0x66D9EF, 0xAE81FF, 0xA1EFE4, 0xF9F8F5,
                ],
            ),
            theme(
                "Monokai Light",
                0x272822,
                0xFAF9F5,
                0xE8E8D8,
                [
                    0x272822, 0xC4004E, 0x5E8A00, 0xB57A00, 0x0F7FA8, 0x7D4FD1, 0x0E8F84, 0x75715E,
                    0x75715E, 0xF92672, 0x7AA000, 0xD49A1A, 0x1FA0CF, 0x9A6FF0, 0x25B0A5, 0x272822,
                ],
            ),
        ]
    })
}

/// The built-in theme of this name.
pub fn get(name: &str) -> Option<&'static Theme> {
    all().iter().find(|theme| theme.name == name)
}
