//! Terminal colour themes: reading the files of Alacritty, kitty and Ghostty, and the
//! library of themes the user has imported.

use std::path::Path;

use iced::Color;
use serde::{Deserialize, Serialize};

use crate::terminal_view::Palette;

/// A colour as `#rrggbb` text, in files and in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rgb(u32);

impl Rgb {
    /// Reads `#rrggbb`, `0xrrggbb` or `rrggbb`.
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_matches(|c| c == '"' || c == '\'');
        let digits = text
            .strip_prefix('#')
            .or_else(|| text.strip_prefix("0x"))
            .or_else(|| text.strip_prefix("0X"))
            .unwrap_or(text);
        if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(digits, 16).ok().map(Rgb)
    }

    fn color(self) -> Color {
        crate::terminal_view::rgb(self.0)
    }

    /// How bright the colour looks, from 0 to 1.
    fn luminance(self) -> f32 {
        let [red, green, blue] = [16, 8, 0].map(|shift| ((self.0 >> shift) & 0xFF) as f32 / 255.0);
        0.299 * red + 0.587 * green + 0.114 * blue
    }
}

impl TryFrom<String> for Rgb {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        Rgb::parse(&text).ok_or_else(|| format!("{text} is not a colour like #1d2021"))
    }
}

impl From<Rgb> for String {
    fn from(color: Rgb) -> String {
        format!("#{:06x}", color.0)
    }
}

/// A named set of terminal colours.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub foreground: Rgb,
    pub background: Rgb,
    /// Behind selected text; a theme may leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<Rgb>,
    /// The 16 colours programs ask for by number: eight normal, then eight bright.
    pub ansi: Vec<Rgb>,
}

impl Theme {
    /// The colours to draw with. What the theme does not say comes from the default
    /// palette that suits its background.
    pub fn palette(&self) -> Palette {
        let fallback = Palette::default_for(self.background.luminance() < 0.5);
        let mut ansi = fallback.ansi;
        for (slot, color) in ansi.iter_mut().zip(&self.ansi) {
            *slot = color.color();
        }
        Palette {
            foreground: self.foreground.color(),
            background: self.background.color(),
            selection: self.selection.map_or(fallback.selection, Rgb::color),
            found: fallback.found,
            found_current: fallback.found_current,
            ansi,
        }
    }
}

/// Reads a theme file of Alacritty (`.toml`), kitty (`.conf`) or Ghostty (a file with
/// `palette = ` lines). `file_name` gives the theme its name and hints at the format.
pub fn parse(file_name: &str, text: &str) -> Result<Theme, String> {
    let path = Path::new(file_name);
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Imported theme")
        .to_string();
    let is_toml = path.extension().is_some_and(|ext| ext == "toml");
    let is_ghostty = text
        .lines()
        .any(|line| line.trim_start().starts_with("palette") && line.contains('='));
    let found = if is_toml || text.contains("[colors") {
        read_alacritty(text)?
    } else if is_ghostty {
        read_ghostty(text)
    } else {
        read_kitty(text)
    };
    found.into_theme(name)
}

/// What a file said, before it is checked.
#[derive(Default)]
struct Found {
    foreground: Option<Rgb>,
    background: Option<Rgb>,
    selection: Option<Rgb>,
    ansi: [Option<Rgb>; 16],
}

impl Found {
    fn into_theme(self, name: String) -> Result<Theme, String> {
        let foreground = self.foreground.ok_or("the file has no foreground colour")?;
        let background = self.background.ok_or("the file has no background colour")?;
        if self.ansi.iter().all(Option::is_none) {
            return Err("the file has none of the 16 colours".to_string());
        }
        // Colours the file leaves out are taken from the default palette, and written as such.
        let fallback = Palette::default_for(background.luminance() < 0.5).ansi;
        let ansi = self
            .ansi
            .iter()
            .zip(fallback)
            .map(|(given, default)| given.unwrap_or_else(|| from_color(default)))
            .collect();
        Ok(Theme {
            name,
            foreground,
            background,
            selection: self.selection,
            ansi,
        })
    }
}

fn from_color(color: Color) -> Rgb {
    let [red, green, blue, _] = color.into_rgba8();
    Rgb(u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue))
}

const ALACRITTY_NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

fn read_alacritty(text: &str) -> Result<Found, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|error: toml::de::Error| error.message().to_string())?;
    let colors = table
        .get("colors")
        .and_then(toml::Value::as_table)
        .ok_or("the file has no [colors] table")?;
    let color = |table: &str, key: &str| {
        colors
            .get(table)
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get(key))
            .and_then(toml::Value::as_str)
            .and_then(Rgb::parse)
    };
    let mut found = Found {
        foreground: color("primary", "foreground"),
        background: color("primary", "background"),
        selection: color("selection", "background"),
        ..Found::default()
    };
    for (index, name) in ALACRITTY_NAMES.iter().enumerate() {
        found.ansi[index] = color("normal", name);
        found.ansi[index + 8] = color("bright", name);
    }
    Ok(found)
}

/// `key value` lines; a line that starts with `#` is a comment.
fn read_kitty(text: &str) -> Found {
    let mut found = Found::default();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let value = Rgb::parse(value.trim());
        match key {
            "foreground" => found.foreground = value,
            "background" => found.background = value,
            "selection_background" => found.selection = value,
            _ => {
                if let Some(index) = key
                    .strip_prefix("color")
                    .and_then(|number| number.parse::<usize>().ok())
                    .filter(|index| *index < 16)
                {
                    found.ansi[index] = value;
                }
            }
        }
    }
    found
}

/// `key = value` lines, with the colours of the palette as `palette = 4=#6AA8FF`.
fn read_ghostty(text: &str) -> Found {
    let mut found = Found::default();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "foreground" => found.foreground = Rgb::parse(value),
            "background" => found.background = Rgb::parse(value),
            "selection-background" => found.selection = Rgb::parse(value),
            "palette" => {
                if let Some((index, color)) = value.split_once('=')
                    && let Ok(index) = index.trim().parse::<usize>()
                    && index < 16
                {
                    found.ansi[index] = Rgb::parse(color);
                }
            }
            _ => {}
        }
    }
    found
}

/// The themes the user has imported, kept in `terminal-themes.toml` beside the settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Library {
    #[serde(default, rename = "theme")]
    pub themes: Vec<Theme>,
}

impl Library {
    pub fn get(&self, name: &str) -> Option<&Theme> {
        self.themes.iter().find(|theme| theme.name == name)
    }

    /// Adds a theme, replacing one of the same name. The list stays sorted by name.
    pub fn add(&mut self, theme: Theme) {
        self.themes.retain(|known| known.name != theme.name);
        self.themes.push(theme);
        self.themes
            .sort_by_cached_key(|theme| theme.name.to_lowercase());
    }

    pub fn remove(&mut self, name: &str) {
        self.themes.retain(|theme| theme.name != name);
    }
}

/// Where the library lives: beside the connections file.
pub fn path_beside(connections_file: &Path) -> std::path::PathBuf {
    connections_file.with_file_name("terminal-themes.toml")
}

/// A missing file is an empty library. A broken one is an error, so it is not overwritten.
pub fn load(path: &Path) -> Result<Library, String> {
    if !path.exists() {
        return Ok(Library::default());
    }
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    toml::from_str(&text).map_err(|error| error.to_string())
}

pub fn save(library: &Library, path: &Path) -> Result<(), String> {
    let text = toml::to_string_pretty(library).map_err(|error| error.to_string())?;
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, text).map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "terminal_theme_tests.rs"]
mod tests;
