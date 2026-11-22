//! The look of the app: every colour, radius and size lives here.
//! Change a value in this file, never in a screen.

use std::borrow::Borrow;

use iced::border::{self, Border};
use iced::theme::{Mode, Palette};
use iced::widget::overlay::menu;
use iced::widget::{
    PickList, TextInput, button, checkbox, container, pick_list, space, text, text_input,
};
use iced::{Color, Font, Shadow, Theme, Vector, color};
use iced::{Element, Length};

pub const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
pub const INTER_BOLD: &[u8] = include_bytes!("../assets/fonts/Inter-Bold.ttf");
pub const FONT: Font = Font::with_name("Inter");

/// Gap between the floating panels, and between them and the window edge.
pub const GAP: f32 = 12.0;
/// Padding inside a panel.
pub const PANEL_PADDING: f32 = 16.0;
const RADIUS_PANEL: f32 = 16.0;
const RADIUS_CONTROL: f32 = 10.0;
const RADIUS_PILL: f32 = 999.0;

/// Font sizes in pixels.
pub const TEXT_SMALL: f32 = 12.0;
pub const TEXT_BODY: f32 = 14.0;
pub const TEXT_TITLE: f32 = 16.0;
pub const TEXT_DISPLAY: f32 = 20.0;

/// The colours of one mode.
pub struct Tokens {
    /// Behind the panels.
    background: Color,
    panel: Color,
    panel_border: Color,
    shadow: Color,
    /// Inputs and secondary buttons at rest.
    surface: Color,
    surface_hover: Color,
    surface_border: Color,
    /// The selected row or tab.
    selected: Color,
    selected_text: Color,
    text: Color,
    text_secondary: Color,
    accent: Color,
    accent_hover: Color,
    on_accent: Color,
    danger: Color,
    on_danger: Color,
}

const fn alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

const DARK: Tokens = Tokens {
    background: color!(0x0F1115),
    panel: color!(0x171A21),
    panel_border: alpha(color!(0xFFFFFF), 0.10),
    shadow: alpha(color!(0x000000), 0.35),
    surface: alpha(color!(0xFFFFFF), 0.05),
    surface_hover: alpha(color!(0xFFFFFF), 0.10),
    surface_border: alpha(color!(0xFFFFFF), 0.10),
    selected: alpha(color!(0x6AA8FF), 0.16),
    selected_text: color!(0x9CC4FF),
    text: color!(0xEEF1F5),
    text_secondary: color!(0xA9B1BE),
    accent: color!(0x6AA8FF),
    accent_hover: color!(0x8BBBFF),
    on_accent: color!(0x0A1324),
    danger: color!(0xFF8A8A),
    on_danger: color!(0x2A0A0A),
};

const LIGHT: Tokens = Tokens {
    background: color!(0xF4F5F7),
    panel: color!(0xFFFFFF),
    panel_border: alpha(color!(0x0F172A), 0.08),
    shadow: alpha(color!(0x0F172A), 0.10),
    surface: alpha(color!(0x0F172A), 0.04),
    surface_hover: alpha(color!(0x0F172A), 0.08),
    surface_border: alpha(color!(0x0F172A), 0.10),
    selected: alpha(color!(0x2563EB), 0.12),
    selected_text: color!(0x1E40AF),
    text: color!(0x14161A),
    text_secondary: color!(0x4B5563),
    accent: color!(0x2563EB),
    accent_hover: color!(0x1D4ED8),
    on_accent: color!(0xFFFFFF),
    danger: color!(0xB91C1C),
    on_danger: color!(0xFFFFFF),
};

/// Black and white with hard borders: every text and border colour is at least 7:1 on its panel.
const DARK_CONTRAST: Tokens = Tokens {
    background: color!(0x000000),
    panel: color!(0x000000),
    panel_border: color!(0xFFFFFF),
    shadow: alpha(color!(0x000000), 0.0),
    surface: alpha(color!(0xFFFFFF), 0.12),
    surface_hover: alpha(color!(0xFFFFFF), 0.25),
    surface_border: color!(0xFFFFFF),
    selected: alpha(color!(0xFFFF00), 0.25),
    selected_text: color!(0xFFFF66),
    text: color!(0xFFFFFF),
    text_secondary: color!(0xE6E6E6),
    accent: color!(0x7DB8FF),
    accent_hover: color!(0xA8D0FF),
    on_accent: color!(0x000000),
    danger: color!(0xFF9C9C),
    on_danger: color!(0x000000),
};

const LIGHT_CONTRAST: Tokens = Tokens {
    background: color!(0xFFFFFF),
    panel: color!(0xFFFFFF),
    panel_border: color!(0x000000),
    shadow: alpha(color!(0x000000), 0.0),
    surface: alpha(color!(0x000000), 0.06),
    surface_hover: alpha(color!(0x000000), 0.16),
    surface_border: color!(0x000000),
    selected: alpha(color!(0x0036B3), 0.18),
    selected_text: color!(0x00227A),
    text: color!(0x000000),
    text_secondary: color!(0x1F1F1F),
    accent: color!(0x0036B3),
    accent_hover: color!(0x00227A),
    on_accent: color!(0xFFFFFF),
    danger: color!(0x8A0000),
    on_danger: color!(0xFFFFFF),
};

const HIGH_CONTRAST_SUFFIX: &str = " High Contrast";

/// The app theme for a light or dark system. An unknown mode is treated as light.
pub fn for_mode(mode: Mode, high_contrast: bool) -> Theme {
    let (name, tokens) = match (mode, high_contrast) {
        (Mode::Dark, false) => ("Remotething Dark", &DARK),
        (Mode::Dark, true) => ("Remotething Dark High Contrast", &DARK_CONTRAST),
        (Mode::Light | Mode::None, false) => ("Remotething Light", &LIGHT),
        (Mode::Light | Mode::None, true) => ("Remotething Light High Contrast", &LIGHT_CONTRAST),
    };
    let palette = Palette {
        background: tokens.background,
        text: tokens.text,
        primary: tokens.accent,
        success: tokens.accent,
        warning: tokens.danger,
        danger: tokens.danger,
    };
    Theme::custom(name, palette)
}

fn tokens(theme: &Theme) -> &'static Tokens {
    let high_contrast = theme.to_string().ends_with(HIGH_CONTRAST_SUFFIX);
    match (theme.extended_palette().is_dark, high_contrast) {
        (true, false) => &DARK,
        (false, false) => &LIGHT,
        (true, true) => &DARK_CONTRAST,
        (false, true) => &LIGHT_CONTRAST,
    }
}

/// A floating panel: rounded, hairline border, soft shadow.
pub fn panel(theme: &Theme) -> container::Style {
    let tokens = tokens(theme);
    container::Style {
        text_color: Some(tokens.text),
        background: Some(tokens.panel.into()),
        border: Border {
            color: tokens.panel_border,
            width: 1.0,
            radius: RADIUS_PANEL.into(),
        },
        shadow: Shadow {
            color: tokens.shadow,
            offset: Vector::new(0.0, 8.0),
            blur_radius: 28.0,
        },
        ..container::Style::default()
    }
}

/// A row in a list. The selected one carries the accent tint.
pub fn list_row(is_selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let tokens = tokens(theme);
        if !is_selected {
            return container::Style::default();
        }
        container::Style {
            text_color: Some(tokens.selected_text),
            background: Some(tokens.selected.into()),
            border: border::rounded(RADIUS_CONTROL),
            ..container::Style::default()
        }
    }
}

/// Something small that floats over the window, such as a menu: a panel with a tighter shadow.
pub fn floating(theme: &Theme) -> container::Style {
    container::Style {
        shadow: Shadow {
            color: tokens(theme).shadow,
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..panel(theme)
    }
}

/// The size shown while the window is resized: see-through, so what is behind shows.
pub fn size_badge(theme: &Theme) -> container::Style {
    let tokens = tokens(theme);
    container::Style {
        text_color: Some(tokens.text),
        background: Some(
            Color {
                a: 0.8,
                ..tokens.panel
            }
            .into(),
        ),
        border: Border {
            color: tokens.panel_border,
            width: 1.0,
            radius: RADIUS_CONTROL.into(),
        },
        ..container::Style::default()
    }
}

/// A tab in the bar: tinted when it is the one in front, quiet otherwise.
pub fn tab(is_active: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let tokens = tokens(theme);
        let (background, text_color) = if is_active {
            (tokens.selected, tokens.selected_text)
        } else {
            (tokens.surface, tokens.text)
        };
        container::Style {
            text_color: Some(text_color),
            background: Some(background.into()),
            border: border::rounded(RADIUS_CONTROL),
            ..container::Style::default()
        }
    }
}

/// The frame of a pane of the split view: an accent edge on the one that has the keyboard.
pub fn pane_frame(has_keyboard: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let tokens = tokens(theme);
        let color = if has_keyboard {
            tokens.accent
        } else {
            Color::TRANSPARENT
        };
        container::Style {
            border: Border {
                color,
                width: 1.5,
                radius: RADIUS_CONTROL.into(),
            },
            ..container::Style::default()
        }
    }
}

/// The tab being carried: raised above the bar with a shadow and an accent edge.
pub fn carried_tab(theme: &Theme) -> container::Style {
    let tokens = tokens(theme);
    container::Style {
        text_color: Some(tokens.selected_text),
        background: Some(tokens.selected.into()),
        border: Border {
            color: tokens.accent,
            width: 1.0,
            radius: RADIUS_CONTROL.into(),
        },
        shadow: Shadow {
            color: tokens.shadow,
            offset: Vector::new(0.0, 3.0),
            blur_radius: 8.0,
        },
        ..container::Style::default()
    }
}

/// A tab that was just dropped: its accent edge fades out as `glow` goes from 1 to 0.
pub fn glowing_tab(is_active: bool, glow: f32) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let tokens = tokens(theme);
        let mut style = tab(is_active)(theme);
        style.border = Border {
            color: Color {
                a: glow.clamp(0.0, 1.0),
                ..tokens.accent
            },
            width: 2.0,
            radius: RADIUS_CONTROL.into(),
        };
        style
    }
}

/// The row a dragged item would be dropped on.
pub fn drop_target(theme: &Theme) -> container::Style {
    let tokens = tokens(theme);
    container::Style {
        text_color: Some(tokens.selected_text),
        background: Some(tokens.selected.into()),
        border: Border {
            color: tokens.accent,
            width: 1.5,
            radius: RADIUS_CONTROL.into(),
        },
        ..container::Style::default()
    }
}

/// A small label on a row, such as the protocol of a connection.
pub fn chip(theme: &Theme) -> container::Style {
    let tokens = tokens(theme);
    container::Style {
        text_color: Some(tokens.text_secondary),
        background: Some(tokens.surface.into()),
        border: border::rounded(6.0),
        ..container::Style::default()
    }
}

/// The main action of a screen.
pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    let tokens = tokens(theme);
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => tokens.accent_hover,
        button::Status::Active => tokens.accent,
        button::Status::Disabled => alpha(tokens.accent, 0.5),
    };
    button::Style {
        background: Some(background.into()),
        text_color: tokens.on_accent,
        border: border::rounded(RADIUS_CONTROL),
        ..button::Style::default()
    }
}

/// A quiet, pill-shaped button for everything that is not the main action.
pub fn pill(theme: &Theme, status: button::Status) -> button::Style {
    let tokens = tokens(theme);
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => tokens.surface_hover,
        _ => tokens.surface,
    };
    button::Style {
        background: Some(background.into()),
        text_color: tokens.text,
        border: Border {
            color: tokens.surface_border,
            width: 1.0,
            radius: RADIUS_PILL.into(),
        },
        ..button::Style::default()
    }
}

/// A button for an action that removes something.
pub fn danger(theme: &Theme, status: button::Status) -> button::Style {
    let tokens = tokens(theme);
    let is_active = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: is_active.then(|| tokens.danger.into()),
        text_color: if is_active {
            tokens.on_danger
        } else {
            tokens.danger
        },
        border: Border {
            color: tokens.danger,
            width: 1.0,
            radius: RADIUS_CONTROL.into(),
        },
        ..button::Style::default()
    }
}

/// A list entry or tab that is a button: no fill until hovered, accent tint when selected.
pub fn entry(is_selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let tokens = tokens(theme);
        let is_hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let (background, text_color) = if is_selected {
            (Some(tokens.selected.into()), tokens.selected_text)
        } else if is_hovered {
            (Some(tokens.surface_hover.into()), tokens.text)
        } else {
            (None, tokens.text)
        };
        button::Style {
            background,
            text_color,
            border: border::rounded(RADIUS_CONTROL),
            ..button::Style::default()
        }
    }
}

/// A checkbox whose empty box shows clearly against the panel.
pub fn check(theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let tokens = tokens(theme);
    let (checkbox::Status::Active { is_checked }
    | checkbox::Status::Hovered { is_checked }
    | checkbox::Status::Disabled { is_checked }) = status;
    let hovered = matches!(status, checkbox::Status::Hovered { .. });
    let (fill, edge) = match (is_checked, hovered) {
        (true, true) => (tokens.accent_hover, tokens.accent_hover),
        (true, false) => (tokens.accent, tokens.accent),
        (false, true) => (tokens.surface_hover, tokens.text),
        (false, false) => (tokens.surface, tokens.text_secondary),
    };
    checkbox::Style {
        background: fill.into(),
        icon_color: tokens.on_accent,
        border: Border {
            color: edge,
            width: 1.5,
            radius: 4.0.into(),
        },
        text_color: Some(tokens.text),
    }
}

/// A list entry with a hairline outline, so neighbours read as separate items.
pub fn outlined_entry(is_selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let style = entry(is_selected)(theme, status);
        button::Style {
            border: Border {
                color: tokens(theme).surface_border,
                width: 1.0,
                radius: RADIUS_CONTROL.into(),
            },
            ..style
        }
    }
}

/// A thin vertical line between two columns of a panel.
pub fn vertical_divider<'a, Message: 'a>() -> Element<'a, Message> {
    container(space::horizontal())
        .width(Length::Fixed(1.0))
        .height(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(tokens(theme).surface_border.into()),
            ..container::Style::default()
        })
        .into()
}

/// Text that explains or labels, one step quieter than body text.
pub fn secondary_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_secondary),
    }
}

/// A text field in the app's style.
pub fn input<'a, Message: Clone + 'a>(placeholder: &str, value: &str) -> TextInput<'a, Message> {
    text_input(placeholder, value)
        .padding(8)
        .size(TEXT_BODY)
        .style(input_style)
}

fn input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let tokens = tokens(theme);
    let border_color = match status {
        text_input::Status::Focused { .. } => tokens.accent,
        _ => tokens.surface_border,
    };
    text_input::Style {
        background: tokens.surface.into(),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: RADIUS_CONTROL.into(),
        },
        icon: tokens.text_secondary,
        placeholder: alpha(tokens.text_secondary, 0.7),
        value: tokens.text,
        selection: tokens.selected,
    }
}

/// A drop-down in the app's style.
pub fn pick<'a, T, L, V, Message>(
    options: L,
    selected: Option<V>,
    on_selected: impl Fn(T) -> Message + 'a,
) -> PickList<'a, T, L, V, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    L: Borrow<[T]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone,
{
    pick_list(options, selected, on_selected)
        .padding(8)
        .text_size(TEXT_BODY)
        .style(pick_style)
        .menu_style(menu_style)
}

fn pick_style(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let tokens = tokens(theme);
    let border_color = match status {
        pick_list::Status::Opened { .. } => tokens.accent,
        _ => tokens.surface_border,
    };
    pick_list::Style {
        text_color: tokens.text,
        placeholder_color: tokens.text_secondary,
        handle_color: tokens.text_secondary,
        background: tokens.surface.into(),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: RADIUS_CONTROL.into(),
        },
    }
}

fn menu_style(theme: &Theme) -> menu::Style {
    let tokens = tokens(theme);
    menu::Style {
        background: tokens.panel.into(),
        border: Border {
            color: tokens.panel_border,
            width: 1.0,
            radius: RADIUS_CONTROL.into(),
        },
        text_color: tokens.text,
        selected_text_color: tokens.selected_text,
        selected_background: tokens.selected.into(),
        shadow: Shadow {
            color: tokens.shadow,
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of an opaque colour.
    fn luminance(color: Color) -> f32 {
        let channel = |value: f32| {
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
    }

    fn contrast(first: Color, second: Color) -> f32 {
        let (lighter, darker) = if luminance(first) > luminance(second) {
            (luminance(first), luminance(second))
        } else {
            (luminance(second), luminance(first))
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn text_is_readable_on_panels_in_both_modes() {
        for tokens in [&LIGHT, &DARK] {
            assert!(contrast(tokens.text, tokens.panel) >= 7.0);
            assert!(contrast(tokens.text_secondary, tokens.panel) >= 4.5);
            assert!(contrast(tokens.danger, tokens.panel) >= 4.5);
        }
    }

    #[test]
    fn high_contrast_text_and_borders_reach_seven_to_one() {
        for tokens in [&LIGHT_CONTRAST, &DARK_CONTRAST] {
            assert!(contrast(tokens.text, tokens.panel) >= 7.0);
            assert!(contrast(tokens.text_secondary, tokens.panel) >= 7.0);
            assert!(contrast(tokens.danger, tokens.panel) >= 7.0);
            assert!(contrast(tokens.accent, tokens.panel) >= 4.5);
            assert!(contrast(tokens.selected_text, tokens.panel) >= 7.0);
            assert!(contrast(tokens.panel_border, tokens.panel) >= 7.0);
            assert!(contrast(tokens.surface_border, tokens.panel) >= 7.0);
            assert!(contrast(tokens.on_accent, tokens.accent) >= 7.0);
            assert!(contrast(tokens.on_danger, tokens.danger) >= 7.0);
        }
    }

    #[test]
    fn the_high_contrast_theme_keeps_its_mode_and_tokens() {
        let dark = for_mode(Mode::Dark, true);
        let light = for_mode(Mode::Light, true);

        assert!(dark.extended_palette().is_dark);
        assert!(!light.extended_palette().is_dark);
        assert_eq!(tokens(&dark).panel_border, DARK_CONTRAST.panel_border);
        assert_eq!(tokens(&light).panel_border, LIGHT_CONTRAST.panel_border);
    }

    #[test]
    fn button_labels_are_readable_on_their_fill() {
        for tokens in [&LIGHT, &DARK] {
            assert!(contrast(tokens.on_accent, tokens.accent) >= 4.5);
            assert!(contrast(tokens.on_danger, tokens.danger) >= 4.5);
        }
    }

    #[test]
    fn an_unknown_system_mode_gets_the_light_theme() {
        assert!(!for_mode(Mode::None, false).extended_palette().is_dark);
        assert!(!for_mode(Mode::Light, false).extended_palette().is_dark);
        assert!(for_mode(Mode::Dark, false).extended_palette().is_dark);
    }
}
