//! Draws a terminal screen on a canvas and reports how many characters fit.

use std::time::{Duration, Instant};

use iced::widget::canvas::{self, Cache, Geometry};
use iced::widget::{Action, container};
use iced::{Color, Element, Font, Length, Point, Rectangle, Renderer, Size, Theme, font, mouse};
use remotething_remote::ssh::TerminalSize;

use crate::terminal::{Cell, Granularity, Paint, Screen};

pub const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/NotoSansMono-Regular.ttf");
pub const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSansMono-Bold.ttf");
pub(crate) const FONT: Font = Font::with_name("Noto Sans Mono");

/// The text size when the user has not chosen one.
pub const DEFAULT_FONT_SIZE: u8 = 15;
/// The smallest and largest text size to choose.
pub const FONT_SIZES: std::ops::RangeInclusive<u8> = 10..=24;
const PADDING: f32 = 8.0;
/// Wheel events closer together than this count as one notch.
const ZOOM_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(30);
/// Lines scrolled per notch of the mouse wheel.
const WHEEL_LINES: f32 = 3.0;

/// How big the characters are, and so how much room each takes.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Metrics {
    font: Font,
    font_size: f32,
    /// The bundled font is 0.6 em wide, so every character takes exactly this much.
    cell_width: f32,
    cell_height: f32,
}

impl Metrics {
    fn new(font_size: u8) -> Self {
        let font_size = f32::from(font_size);
        Self {
            font: FONT,
            font_size,
            cell_width: font_size * 0.6,
            cell_height: (font_size * 4.0 / 3.0).round(),
        }
    }

    fn with_font(self, font: Font) -> Self {
        Self { font, ..self }
    }

    /// How far below the top of a line an underline goes.
    fn underline_offset(self) -> f32 {
        (self.cell_height * 0.85).round()
    }
}

/// What the user chose for terminals: colours and text size.
#[derive(Debug, Clone, Copy)]
struct Look {
    /// `None` follows the light or dark theme of the app.
    palette: Option<Palette>,
    font_size: u8,
    font: Font,
}

const DEFAULT_LOOK: Look = Look {
    palette: None,
    font_size: DEFAULT_FONT_SIZE,
    font: FONT,
};

static LOOK: std::sync::RwLock<Look> = std::sync::RwLock::new(DEFAULT_LOOK);

/// Font names handed to iced, which wants them for as long as the program runs. Each
/// distinct name is kept once.
static FONT_NAMES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// The font of an installed family, or the bundled one when `family` is `None` or blank.
/// A family that is not installed is drawn in whatever font the system falls back to.
pub(crate) fn font_named(family: Option<&str>) -> Font {
    let Some(family) = family.map(str::trim).filter(|name| !name.is_empty()) else {
        return FONT;
    };
    let Ok(mut names) = FONT_NAMES.lock() else {
        return FONT;
    };
    let name = match names.iter().find(|known| **known == family) {
        Some(known) => *known,
        None => {
            let leaked: &'static str = Box::leak(family.to_string().into_boxed_str());
            names.push(leaked);
            leaked
        }
    };
    Font::with_name(name)
}

/// Sets how terminals look. Drawings made before need clearing to show it.
pub(crate) fn set_look(palette: Option<Palette>, font_size: u8, font_family: Option<&str>) {
    if let Ok(mut look) = LOOK.write() {
        *look = Look {
            palette,
            font_size: font_size.clamp(*FONT_SIZES.start(), *FONT_SIZES.end()),
            font: font_named(font_family),
        };
    }
}

fn look() -> Look {
    LOOK.read().map(|look| *look).unwrap_or(DEFAULT_LOOK)
}

/// The colours of a terminal: defaults and the 16 a program can ask for by number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Palette {
    pub(crate) foreground: Color,
    pub(crate) background: Color,
    /// Behind selected text.
    pub(crate) selection: Color,
    /// Behind the matches of a search, and behind the one the user is on.
    pub(crate) found: Color,
    pub(crate) found_current: Color,
    pub(crate) ansi: [Color; 16],
}

pub(crate) const fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub(crate) const DARK: Palette = Palette {
    foreground: rgb(0xD7DCE5),
    background: rgb(0x10141C),
    selection: rgb(0x2C4A7A),
    found: rgb(0x5C4B16),
    found_current: rgb(0xA87B14),
    ansi: [
        rgb(0x2B3240),
        rgb(0xF07178),
        rgb(0x8FD694),
        rgb(0xE6C46B),
        rgb(0x6AA8FF),
        rgb(0xC792EA),
        rgb(0x5CCFE6),
        rgb(0xC5CAD3),
        rgb(0x5C6577),
        rgb(0xFF8B92),
        rgb(0xA9E5AD),
        rgb(0xF2D58A),
        rgb(0x8BBBFF),
        rgb(0xD9B0F2),
        rgb(0x8ADDEE),
        rgb(0xFFFFFF),
    ],
};

pub(crate) const LIGHT: Palette = Palette {
    foreground: rgb(0x1F2430),
    background: rgb(0xFFFFFF),
    selection: rgb(0xCFE0FF),
    found: rgb(0xFFE9A6),
    found_current: rgb(0xFFB84D),
    ansi: [
        rgb(0x1F2430),
        rgb(0xC0392B),
        rgb(0x2E7D32),
        rgb(0x9A6700),
        rgb(0x2563EB),
        rgb(0x8E44AD),
        rgb(0x0E7490),
        rgb(0x6B7280),
        rgb(0x4B5563),
        rgb(0xE5484D),
        rgb(0x3F9142),
        rgb(0xB7791F),
        rgb(0x3B82F6),
        rgb(0xA855F7),
        rgb(0x0891B2),
        rgb(0x9CA3AF),
    ],
};

impl Palette {
    /// The colours for a light or dark app theme.
    pub(crate) fn default_for(dark: bool) -> Palette {
        if dark { DARK } else { LIGHT }
    }

    /// The colours chosen by the user, or else those that go with the app theme.
    fn of(theme: &Theme) -> Palette {
        look()
            .palette
            .unwrap_or_else(|| Palette::default_for(theme.extended_palette().is_dark))
    }

    pub(crate) fn color(&self, paint: Paint) -> Color {
        match paint {
            Paint::Foreground => self.foreground,
            Paint::Background => self.background,
            Paint::Ansi(index) => self.ansi[usize::from(index) % self.ansi.len()],
            Paint::Rgb(red, green, blue) => Color::from_rgb8(red, green, blue),
        }
    }
}

/// How many characters fit into a view of this size.
fn size_for(area: Size, metrics: Metrics) -> TerminalSize {
    let columns = ((area.width - 2.0 * PADDING) / metrics.cell_width).floor();
    let lines = ((area.height - 2.0 * PADDING) / metrics.cell_height).floor();
    TerminalSize {
        // A float-to-integer cast saturates, so absurd sizes stay in range.
        columns: columns.max(2.0) as u16,
        lines: lines.max(1.0) as u16,
    }
}

/// What the terminal view reports to its session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The space holds a different number of characters than the screen has.
    Resized(TerminalSize),
    /// The wheel turned, by this many lines back.
    Scrolled(i32),
    /// The wheel turned with Ctrl held: 1 for a bigger text size, -1 for a smaller.
    Zoomed(i32),
    /// The mouse button went down on this character.
    SelectionStarted {
        column: usize,
        line: usize,
        /// By characters for a click, words for two clicks, lines for three.
        granularity: Granularity,
    },
    /// The pointer moved to this character with the button held.
    SelectionExtended { column: usize, line: usize },
    /// The button was released.
    SelectionEnded,
    /// A middle click: paste what was last selected.
    PasteRequested,
}

/// The screen as a widget that fills its space.
pub fn view<'a, Message: 'a>(
    screen: &'a Screen,
    cache: &'a Cache,
    on_event: fn(Event) -> Message,
) -> Element<'a, Message> {
    let program = Program {
        screen,
        cache,
        on_event,
        metrics: Metrics::new(look().font_size).with_font(look().font),
    };
    let canvas = canvas::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fill);
    container(canvas)
        .style(|theme| container::Style {
            background: Some(Palette::of(theme).background.into()),
            border: iced::border::rounded(10.0),
            ..container::Style::default()
        })
        .into()
}

struct Program<'a, Message> {
    screen: &'a Screen,
    cache: &'a Cache,
    on_event: fn(Event) -> Message,
    metrics: Metrics,
}

/// Whether the mouse button is held down from a press inside the view.
#[derive(Default)]
struct Dragging {
    held: bool,
    /// Whether Ctrl is down, which makes the wheel change the text size.
    ctrl: bool,
    /// When the wheel last changed the size. X11 can report one notch twice.
    last_zoom: Option<Instant>,
    /// The last press, to tell a double or triple click.
    last_click: Option<Press>,
}

/// A press of the left button, to tell a double or triple click.
#[derive(Clone, Copy)]
struct Press {
    at: Instant,
    column: usize,
    line: usize,
    count: u8,
}

/// How close in time presses on the same character must be to count together.
const MULTI_CLICK: Duration = Duration::from_millis(400);

/// The press count for a press now: 1 for a single click, up to 3.
fn press_count(previous: Option<Press>, now: Instant, column: usize, line: usize) -> u8 {
    match previous {
        Some(before)
            if before.column == column
                && before.line == line
                && now.duration_since(before.at) <= MULTI_CLICK =>
        {
            (before.count % 3) + 1
        }
        _ => 1,
    }
}

impl<Message> canvas::Program<Message> for Program<'_, Message> {
    type State = Dragging;

    fn update(
        &self,
        dragging: &mut Dragging,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        let publish = |event: Event| Some(Action::publish((self.on_event)(event)));

        if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers)) = event {
            dragging.ctrl = modifiers.control();
        }
        if let iced::Event::Mouse(mouse_event) = event {
            match mouse_event {
                mouse::Event::WheelScrolled { delta } if cursor.is_over(bounds) => {
                    if dragging.ctrl {
                        let steps = zoom_steps(*delta);
                        let now = Instant::now();
                        let repeat = dragging
                            .last_zoom
                            .is_some_and(|at| now.duration_since(at) < ZOOM_DEBOUNCE);
                        if steps != 0 && !repeat {
                            dragging.last_zoom = Some(now);
                            return publish(Event::Zoomed(steps)).map(Action::and_capture);
                        }
                        return None;
                    }
                    let lines = wheel_lines(*delta, self.metrics);
                    if lines != 0 {
                        return publish(Event::Scrolled(lines)).map(Action::and_capture);
                    }
                }
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if let Some(position) = cursor.position_in(bounds) {
                        dragging.held = true;
                        let (column, line) = cell_at(position, self.metrics);
                        let now = Instant::now();
                        let count = press_count(dragging.last_click, now, column, line);
                        let granularity = match count {
                            1 => Granularity::Character,
                            2 => Granularity::Word,
                            _ => Granularity::Line,
                        };
                        dragging.last_click = Some(Press {
                            at: now,
                            column,
                            line,
                            count,
                        });
                        let started = Event::SelectionStarted {
                            column,
                            line,
                            granularity,
                        };
                        return publish(started).map(Action::and_capture);
                    }
                }
                mouse::Event::CursorMoved { .. } if dragging.held => {
                    // Outside the view the nearest character counts, so a drag can overshoot.
                    if let Some(position) = cursor.position_from(bounds.position()) {
                        let (column, line) = cell_at(position, self.metrics);
                        return publish(Event::SelectionExtended { column, line });
                    }
                }
                mouse::Event::ButtonPressed(mouse::Button::Middle) if cursor.is_over(bounds) => {
                    return publish(Event::PasteRequested).map(Action::and_capture);
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) if dragging.held => {
                    dragging.held = false;
                    return publish(Event::SelectionEnded);
                }
                _ => {}
            }
        }

        let fits = size_for(bounds.size(), self.metrics);
        let has = (self.screen.columns(), self.screen.lines());
        if has == (usize::from(fits.columns), usize::from(fits.lines)) {
            return None;
        }
        publish(Event::Resized(fits))
    }

    fn draw(
        &self,
        _dragging: &Dragging,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let palette = &Palette::of(theme);
        let metrics = self.metrics;
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let cell = Size::new(metrics.cell_width, metrics.cell_height);
            let cells = self.screen.cells();
            for cell_drawn in &cells {
                if cell_drawn.background != Paint::Background {
                    let color = palette.color(cell_drawn.background);
                    frame.fill_rectangle(
                        origin(cell_drawn.column, cell_drawn.line, metrics),
                        cell,
                        color,
                    );
                }
            }
            for span in self.screen.selected_spans() {
                let width = (span.end + 1 - span.start) as f32 * metrics.cell_width;
                let size = Size::new(width, metrics.cell_height);
                frame.fill_rectangle(
                    origin(span.start, span.line, metrics),
                    size,
                    palette.selection,
                );
            }
            for found in self.screen.found_spans() {
                let span = found.span;
                let width = (span.end + 1 - span.start) as f32 * metrics.cell_width;
                let color = if found.current {
                    palette.found_current
                } else {
                    palette.found
                };
                let size = Size::new(width, metrics.cell_height);
                frame.fill_rectangle(origin(span.start, span.line, metrics), size, color);
            }
            for run in runs(&cells) {
                draw_run(frame, palette, &run, metrics);
            }
            if let Some((column, line)) = self.screen.cursor() {
                let block = Color {
                    a: 0.5,
                    ..palette.foreground
                };
                frame.fill_rectangle(origin(column, line, metrics), cell, block);
            }
        });
        vec![geometry]
    }

    fn mouse_interaction(
        &self,
        _dragging: &Dragging,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

/// Wheel movement as a text size step: 1 for up, -1 for down, whatever the distance.
fn zoom_steps(delta: mouse::ScrollDelta) -> i32 {
    let (mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. }) = delta;
    if y > 0.0 {
        1
    } else if y < 0.0 {
        -1
    } else {
        0
    }
}

/// Wheel movement as lines to scroll back: three per notch, or the distance a touchpad reports.
fn wheel_lines(delta: mouse::ScrollDelta, metrics: Metrics) -> i32 {
    match delta {
        mouse::ScrollDelta::Lines { y, .. } => (y * WHEEL_LINES).round() as i32,
        mouse::ScrollDelta::Pixels { y, .. } => (y / metrics.cell_height).round() as i32,
    }
}

/// The character under a position inside the view. Positions before the first
/// character count as the first.
fn cell_at(position: Point, metrics: Metrics) -> (usize, usize) {
    let column = ((position.x - PADDING) / metrics.cell_width)
        .floor()
        .max(0.0);
    let line = ((position.y - PADDING) / metrics.cell_height)
        .floor()
        .max(0.0);
    (column as usize, line as usize)
}

fn origin(column: usize, line: usize, metrics: Metrics) -> Point {
    Point::new(
        PADDING + column as f32 * metrics.cell_width,
        PADDING + line as f32 * metrics.cell_height,
    )
}

/// Neighbouring characters of one style, drawn in one go.
#[derive(Debug, PartialEq)]
struct Run {
    column: usize,
    line: usize,
    text: String,
    foreground: Paint,
    bold: bool,
    underline: bool,
}

/// Groups cells into runs. Only ASCII is grouped: any other character may come
/// from a fallback font of another width, so it is placed on its own.
fn runs(cells: &[Cell]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for cell in cells {
        if cell.character == ' ' && !cell.underline {
            continue;
        }
        if let Some(run) = runs.last_mut()
            && run.line == cell.line
            && run.column + run.text.len() == cell.column
            && run.text.is_ascii()
            && cell.character.is_ascii()
            && run.foreground == cell.foreground
            && run.bold == cell.bold
            && run.underline == cell.underline
        {
            run.text.push(cell.character);
            continue;
        }
        runs.push(Run {
            column: cell.column,
            line: cell.line,
            text: cell.character.to_string(),
            foreground: cell.foreground,
            bold: cell.bold,
            underline: cell.underline,
        });
    }
    runs
}

fn draw_run(frame: &mut canvas::Frame, palette: &Palette, run: &Run, metrics: Metrics) {
    let color = palette.color(run.foreground);
    let position = origin(run.column, run.line, metrics);
    let weight = if run.bold {
        font::Weight::Bold
    } else {
        font::Weight::Normal
    };
    frame.fill_text(canvas::Text {
        content: run.text.clone(),
        position,
        color,
        size: metrics.font_size.into(),
        line_height: iced::widget::text::LineHeight::Absolute(metrics.cell_height.into()),
        font: Font {
            weight,
            ..metrics.font
        },
        ..canvas::Text::default()
    });
    if run.underline {
        let width = run.text.chars().count() as f32 * metrics.cell_width;
        let start = Point::new(position.x, position.y + metrics.underline_offset());
        frame.fill_rectangle(start, Size::new(width, 1.0), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const METRICS: Metrics = Metrics {
        font: FONT,
        font_size: 15.0,
        cell_width: 9.0,
        cell_height: 20.0,
    };

    fn cells(bytes: &[u8]) -> Vec<Cell> {
        let mut screen = Screen::new(40, 4);
        screen.feed(bytes);
        screen.cells()
    }

    #[test]
    fn quick_presses_on_one_character_count_up_to_three_and_start_over() {
        let start = Instant::now();
        let press = |count, after_ms: u64| Press {
            at: start + Duration::from_millis(after_ms),
            column: 3,
            line: 1,
            count,
        };

        assert_eq!(press_count(None, start, 3, 1), 1);
        assert_eq!(press_count(Some(press(1, 0)), press(1, 100).at, 3, 1), 2);
        assert_eq!(press_count(Some(press(2, 0)), press(1, 100).at, 3, 1), 3);
        assert_eq!(press_count(Some(press(3, 0)), press(1, 100).at, 3, 1), 1);
    }

    #[test]
    fn a_slow_or_moved_press_is_a_new_click() {
        let start = Instant::now();
        let before = Press {
            at: start,
            column: 3,
            line: 1,
            count: 1,
        };
        let later = start + Duration::from_millis(900);

        assert_eq!(press_count(Some(before), later, 3, 1), 1);
        assert_eq!(press_count(Some(before), start, 4, 1), 1);
    }

    #[test]
    fn the_size_is_what_fits_inside_the_padding() {
        let size = size_for(
            Size::new(2.0 * PADDING + 90.5, 2.0 * PADDING + 45.0),
            METRICS,
        );

        assert_eq!((size.columns, size.lines), (10, 2));
    }

    #[test]
    fn a_tiny_view_still_has_a_usable_size() {
        let size = size_for(Size::new(3.0, 3.0), METRICS);

        assert_eq!((size.columns, size.lines), (2, 1));
    }

    #[test]
    fn the_wheel_zooms_one_step_up_or_down() {
        let notch = |y| mouse::ScrollDelta::Lines { x: 0.0, y };

        assert_eq!(zoom_steps(notch(3.0)), 1);
        assert_eq!(zoom_steps(notch(-0.2)), -1);
        assert_eq!(zoom_steps(notch(0.0)), 0);
    }

    #[test]
    fn a_wheel_notch_up_scrolls_three_lines_back() {
        let up = mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 };
        let down = mouse::ScrollDelta::Pixels { x: 0.0, y: -40.0 };

        assert_eq!(wheel_lines(up, METRICS), 3);
        assert_eq!(wheel_lines(down, METRICS), -2);
    }

    #[test]
    fn a_position_maps_to_the_character_under_it() {
        let inside = Point::new(
            PADDING + 2.5 * METRICS.cell_width,
            PADDING + 1.2 * METRICS.cell_height,
        );

        assert_eq!(cell_at(inside, METRICS), (2, 1));
        assert_eq!(cell_at(Point::new(-30.0, 1.0), METRICS), (0, 0));
    }

    #[test]
    fn a_word_is_one_run() {
        let runs = runs(&cells(b"hello"));

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "hello");
    }

    #[test]
    fn a_gap_or_a_colour_change_starts_a_new_run() {
        let runs = runs(&cells(b"ab cd\x1b[31mef"));

        let texts: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(texts, ["ab", "cd", "ef"]);
        assert_eq!(runs[2].column, 5);
    }

    #[test]
    fn characters_outside_ascii_are_placed_one_by_one() {
        let runs = runs(&cells("a→b".as_bytes()));

        let texts: Vec<&str> = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(texts, ["a", "→", "b"]);
    }
}

#[cfg(test)]
mod metrics_tests {
    use super::*;

    #[test]
    fn the_default_size_keeps_the_cells_it_always_had() {
        let metrics = Metrics::new(DEFAULT_FONT_SIZE);

        assert_eq!((metrics.cell_width, metrics.cell_height), (9.0, 20.0));
        assert_eq!(metrics.underline_offset(), 17.0);
    }

    #[test]
    fn bigger_text_takes_more_room_and_fewer_characters_fit() {
        let small = Metrics::new(12);
        let big = Metrics::new(24);
        let area = Size::new(800.0, 600.0);

        assert!(big.cell_width > small.cell_width && big.cell_height > small.cell_height);
        assert!(size_for(area, big).columns < size_for(area, small).columns);
    }
}
