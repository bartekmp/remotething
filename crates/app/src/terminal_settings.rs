//! The "Terminal" page of the settings: text size, imported colour themes, and a preview
//! of how a theme looks before it is applied.

use std::path::{Path, PathBuf};

use iced::widget::{button, column, container, rich_text, row, scrollable, slider, span, text};
use iced::{Element, Font, Length, font};

use crate::i18n::fl;
use crate::preferences::Preferences;
use crate::terminal::Paint;
use crate::terminal_theme::{self, Library};
use crate::terminal_view::{self, FONT_SIZES, Palette};
use crate::theme;

/// Files bigger than this are not themes.
const MAX_THEME_FILE: u64 = 256 * 1024;
const PREVIEW_SIZE: f32 = 12.0;

/// The imported themes, and which one the preview shows.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub(crate) library: Library,
    /// The theme in the preview. `None` is the default colours.
    pub(crate) previewed: Option<String>,
    /// What the last import said, good or bad.
    pub(crate) notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// "Import theme file…".
    ImportPressed,
    /// The file chosen for an import, or `None` when the dialog was cancelled.
    FilePicked(Option<PathBuf>),
    /// A theme in the list was clicked: show it in the preview.
    Previewed(Option<String>),
    /// "Use this theme".
    ApplyPressed,
    /// "Delete" on the previewed theme.
    DeletePressed,
    FontSizeChanged(u8),
}

/// What changed, for the window to act on.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The preferences changed and must be saved.
    pub preferences_changed: bool,
    /// The dialog to open for an import.
    pub pick_file: bool,
}

impl State {
    /// The library from the file beside `connections_file`; a broken file is reported and
    /// leaves the library empty.
    pub fn load(connections_file: &Path) -> (Self, Option<String>) {
        match terminal_theme::load(&terminal_theme::path_beside(connections_file)) {
            Ok(library) => (
                Self {
                    library,
                    ..Self::default()
                },
                None,
            ),
            Err(reason) => (Self::default(), Some(reason)),
        }
    }

    /// The colours to draw terminals with: those of the chosen theme, if it still exists.
    pub fn applied_palette(&self, preferences: &Preferences) -> Option<Palette> {
        let name = preferences.terminal.theme.as_deref()?;
        self.library.get(name).map(terminal_theme::Theme::palette)
    }
}

pub fn update(
    state: &mut State,
    preferences: &mut Preferences,
    connections_file: &Path,
    message: Message,
) -> Outcome {
    let mut outcome = Outcome::default();
    match message {
        Message::ImportPressed => outcome.pick_file = true,
        Message::FilePicked(Some(path)) => {
            state.notice = Some(match import(state, connections_file, &path) {
                Ok(name) => {
                    state.previewed = Some(name.clone());
                    fl!("terminal-theme-imported", name = name)
                }
                Err(reason) => fl!("terminal-theme-import-failed", reason = reason),
            });
        }
        Message::FilePicked(None) => {}
        Message::Previewed(name) => state.previewed = name,
        Message::ApplyPressed => {
            preferences.terminal.theme = state.previewed.clone();
            outcome.preferences_changed = true;
        }
        Message::DeletePressed => {
            if let Some(name) = state.previewed.take() {
                state.library.remove(&name);
                state.notice = terminal_theme::save(
                    &state.library,
                    &terminal_theme::path_beside(connections_file),
                )
                .err();
                if preferences.terminal.theme.as_deref() == Some(name.as_str()) {
                    preferences.terminal.theme = None;
                    outcome.preferences_changed = true;
                }
            }
        }
        Message::FontSizeChanged(size) => {
            preferences.terminal.font_size = size;
            outcome.preferences_changed = true;
        }
    }
    outcome
}

/// Reads a theme file into the library and saves the library. Returns the theme's name.
fn import(state: &mut State, connections_file: &Path, path: &Path) -> Result<String, String> {
    let size = std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .len();
    if size > MAX_THEME_FILE {
        return Err(fl!("terminal-theme-too-big"));
    }
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let theme = terminal_theme::parse(file_name, &text)?;
    let name = theme.name.clone();
    let mut library = state.library.clone();
    library.add(theme);
    terminal_theme::save(&library, &terminal_theme::path_beside(connections_file))?;
    state.library = library;
    Ok(name)
}

pub fn view<'a>(
    state: &'a State,
    preferences: &'a Preferences,
    dark: bool,
) -> Element<'a, Message> {
    let applied = preferences.terminal.theme.as_deref();
    let size = preferences.terminal.font_size;
    let font_size = column![
        text(fl!("terminal-font-size", size = i64::from(size)))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        slider(FONT_SIZES, size, Message::FontSizeChanged).width(Length::Fixed(240.0)),
    ]
    .spacing(2);

    let choice = |label: String, name: Option<&'a str>| {
        let mark = if name == applied { "✓" } else { "" };
        button(row![text(label).width(Length::Fill), text(mark)])
            .width(Length::Fill)
            .padding([6, 10])
            .style(theme::entry(name == state.previewed.as_deref()))
            .on_press(Message::Previewed(name.map(str::to_string)))
    };
    let mut list = column![choice(fl!("terminal-theme-default"), None)].spacing(2);
    for theme in &state.library.themes {
        list = list.push(choice(theme.name.clone(), Some(theme.name.as_str())));
    }

    let previewed = state.previewed.as_deref();
    let can_apply = previewed != applied;
    let actions = row![
        button(text(fl!("terminal-theme-import")))
            .style(theme::pill)
            .on_press(Message::ImportPressed),
        button(text(fl!("terminal-theme-apply")))
            .style(theme::pill)
            .on_press_maybe(can_apply.then_some(Message::ApplyPressed)),
        button(text(fl!("terminal-theme-delete")))
            .style(theme::pill)
            .on_press_maybe(previewed.map(|_| Message::DeletePressed)),
    ]
    .spacing(8)
    .wrap();

    let palette = previewed
        .and_then(|name| state.library.get(name))
        .map_or_else(
            || Palette::default_for(dark),
            terminal_theme::Theme::palette,
        );

    let left = column![
        text(fl!("terminal-themes"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        container(scrollable(list)).max_height(220),
        actions,
    ]
    .spacing(8)
    .width(Length::Fixed(240.0));

    let mut page = column![
        font_size,
        row![left, preview(&palette)]
            .spacing(theme::GAP * 2.0)
            .wrap()
    ]
    .spacing(16);
    if let Some(notice) = &state.notice {
        page = page.push(text(notice.as_str()).size(theme::TEXT_SMALL));
    }
    page.push(
        text(fl!("terminal-theme-note"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
    )
    .into()
}

/// One piece of text of a preview line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub paint: Paint,
    pub bold: bool,
}

fn seg(text: &str, paint: Paint, bold: bool) -> Segment {
    Segment {
        text: text.to_string(),
        paint,
        bold,
    }
}

const LOGO: [&str; 10] = [
    "       .:////:.       ",
    "    .:oyyyyyyyyo:.    ",
    "  -oyyyysdMMMMdsyyyo- ",
    " .syyyyyyMyyyyMyyyyys.",
    " oyyyyyyyMyyyyMyyyyyyo",
    " oyyyyyyyyMMMMyyyyyyyo",
    " .syyyyyyyyyyyyyyyyys.",
    "  -oyyyyyyyyyyyyyyyo- ",
    "    .:oyyyyyyyyo:.    ",
    "       .:////:.       ",
];

/// A made-up `fastfetch` screen: every default colour, and all 16 of the palette.
pub fn fastfetch() -> Vec<Vec<Segment>> {
    let blue = Paint::Ansi(4);
    let field = |key: &str, value: &str| {
        vec![
            seg(key, blue, true),
            seg(": ", Paint::Foreground, false),
            seg(value, Paint::Foreground, false),
        ]
    };
    let usage = |key: &str, used: &str, percent: &str, paint: Paint| {
        vec![
            seg(key, blue, true),
            seg(&format!(": {used} ("), Paint::Foreground, false),
            seg(percent, paint, false),
            seg(")", Paint::Foreground, false),
        ]
    };
    let swatch = |first: u8| -> Vec<Segment> {
        (first..first + 8)
            .map(|index| seg("███", Paint::Ansi(index), false))
            .collect()
    };
    let info = vec![
        vec![
            seg("user", blue, true),
            seg("@", Paint::Foreground, false),
            seg("workstation", blue, true),
        ],
        vec![seg("------------------", Paint::Foreground, false)],
        field("OS", "Example Linux 24.04 x86_64"),
        field("Host", "Mock Board 1000"),
        field("Kernel", "Linux 6.8.0-generic"),
        field("Uptime", "13 mins"),
        field("Shell", "bash 5.2.21"),
        field("Terminal", "Remotething"),
        field("CPU", "Mock 5 7600 (12) @ 5.1 GHz"),
        usage("Memory", "7.8 GiB / 30.5 GiB", "26%", Paint::Ansi(2)),
        usage("Disk (/)", "579 GiB / 899 GiB", "64%", Paint::Ansi(3)),
        Vec::new(),
        swatch(0),
        swatch(8),
    ];
    info.into_iter()
        .enumerate()
        .map(|(index, mut line)| {
            let logo = LOGO.get(index).copied().unwrap_or("                      ");
            let mut full = vec![seg(logo, blue, true), seg("  ", Paint::Foreground, false)];
            full.append(&mut line);
            full
        })
        .collect()
}

/// A box in the colours of `palette` with the made-up `fastfetch` output.
fn preview<'a>(palette: &Palette) -> Element<'a, Message> {
    let palette = *palette;
    let regular = terminal_view::FONT;
    let bold = Font {
        weight: font::Weight::Bold,
        ..regular
    };
    let lines = fastfetch().into_iter().map(|line| {
        let spans: Vec<_> = line
            .into_iter()
            .map(|segment| {
                span(segment.text)
                    .color(palette.color(segment.paint))
                    .font(if segment.bold { bold } else { regular })
            })
            .collect();
        // An empty line still takes its height.
        let spans = if spans.is_empty() {
            vec![span(" ")]
        } else {
            spans
        };
        rich_text::<(), Message, _, _>(spans)
            .size(PREVIEW_SIZE)
            .wrapping(text::Wrapping::None)
            .into()
    });
    container(column(lines).spacing(1))
        .padding(14)
        .style(move |_| container::Style {
            background: Some(palette.background.into()),
            text_color: Some(palette.foreground),
            border: iced::border::rounded(10.0),
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
#[path = "terminal_settings_tests.rs"]
mod tests;
