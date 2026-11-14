//! Draws a terminal screen on a canvas and reports how many characters fit.

use iced::widget::canvas::{self, Cache, Geometry};
use iced::widget::{Action, container};
use iced::{Color, Element, Font, Length, Point, Rectangle, Renderer, Size, Theme, font, mouse};
use remotething_remote::ssh::TerminalSize;

use crate::terminal::{Cell, Paint, Screen};

pub const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/NotoSansMono-Regular.ttf");
pub const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSansMono-Bold.ttf");
const FONT: Font = Font::with_name("Noto Sans Mono");

const FONT_SIZE: f32 = 15.0;
/// The bundled font is 0.6 em wide, so every character takes exactly this much.
const CELL_WIDTH: f32 = 9.0;
const CELL_HEIGHT: f32 = 20.0;
const PADDING: f32 = 8.0;
const UNDERLINE_OFFSET: f32 = 17.0;
/// Lines scrolled per notch of the mouse wheel.
const WHEEL_LINES: f32 = 3.0;

/// The colours of a terminal: defaults and the 16 a program can ask for by number.
struct Palette {
    foreground: Color,
    background: Color,
    /// Behind selected text.
    selection: Color,
    ansi: [Color; 16],
}

const fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const DARK: Palette = Palette {
    foreground: rgb(0xD7DCE5),
    background: rgb(0x10141C),
    selection: rgb(0x2C4A7A),
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

const LIGHT: Palette = Palette {
    foreground: rgb(0x1F2430),
    background: rgb(0xFFFFFF),
    selection: rgb(0xCFE0FF),
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
    fn of(theme: &Theme) -> &'static Palette {
        if theme.extended_palette().is_dark {
            &DARK
        } else {
            &LIGHT
        }
    }

    fn color(&self, paint: Paint) -> Color {
        match paint {
            Paint::Foreground => self.foreground,
            Paint::Background => self.background,
            Paint::Ansi(index) => self.ansi[usize::from(index) % self.ansi.len()],
            Paint::Rgb(red, green, blue) => Color::from_rgb8(red, green, blue),
        }
    }
}

/// How many characters fit into a view of this size.
pub fn size_for(area: Size) -> TerminalSize {
    let columns = ((area.width - 2.0 * PADDING) / CELL_WIDTH).floor();
    let lines = ((area.height - 2.0 * PADDING) / CELL_HEIGHT).floor();
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
    /// The mouse button went down on this character.
    SelectionStarted { column: usize, line: usize },
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
}

/// Whether the mouse button is held down from a press inside the view.
#[derive(Default)]
struct Dragging(bool);

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

        if let iced::Event::Mouse(mouse_event) = event {
            match mouse_event {
                mouse::Event::WheelScrolled { delta } if cursor.is_over(bounds) => {
                    let lines = wheel_lines(*delta);
                    if lines != 0 {
                        return publish(Event::Scrolled(lines)).map(Action::and_capture);
                    }
                }
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if let Some(position) = cursor.position_in(bounds) {
                        dragging.0 = true;
                        let (column, line) = cell_at(position);
                        let started = Event::SelectionStarted { column, line };
                        return publish(started).map(Action::and_capture);
                    }
                }
                mouse::Event::CursorMoved { .. } if dragging.0 => {
                    // Outside the view the nearest character counts, so a drag can overshoot.
                    if let Some(position) = cursor.position_from(bounds.position()) {
                        let (column, line) = cell_at(position);
                        return publish(Event::SelectionExtended { column, line });
                    }
                }
                mouse::Event::ButtonPressed(mouse::Button::Middle) if cursor.is_over(bounds) => {
                    return publish(Event::PasteRequested).map(Action::and_capture);
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) if dragging.0 => {
                    dragging.0 = false;
                    return publish(Event::SelectionEnded);
                }
                _ => {}
            }
        }

        let fits = size_for(bounds.size());
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
        let palette = Palette::of(theme);
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let cell = Size::new(CELL_WIDTH, CELL_HEIGHT);
            let cells = self.screen.cells();
            for cell_drawn in &cells {
                if cell_drawn.background != Paint::Background {
                    let color = palette.color(cell_drawn.background);
                    frame.fill_rectangle(origin(cell_drawn.column, cell_drawn.line), cell, color);
                }
            }
            for span in self.screen.selected_spans() {
                let width = (span.end + 1 - span.start) as f32 * CELL_WIDTH;
                let size = Size::new(width, CELL_HEIGHT);
                frame.fill_rectangle(origin(span.start, span.line), size, palette.selection);
            }
            for run in runs(&cells) {
                draw_run(frame, palette, &run);
            }
            if let Some((column, line)) = self.screen.cursor() {
                let block = Color {
                    a: 0.5,
                    ..palette.foreground
                };
                frame.fill_rectangle(origin(column, line), cell, block);
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

/// Wheel movement as lines to scroll back: three per notch, or the distance a touchpad reports.
fn wheel_lines(delta: mouse::ScrollDelta) -> i32 {
    match delta {
        mouse::ScrollDelta::Lines { y, .. } => (y * WHEEL_LINES).round() as i32,
        mouse::ScrollDelta::Pixels { y, .. } => (y / CELL_HEIGHT).round() as i32,
    }
}

/// The character under a position inside the view. Positions before the first
/// character count as the first.
fn cell_at(position: Point) -> (usize, usize) {
    let column = ((position.x - PADDING) / CELL_WIDTH).floor().max(0.0);
    let line = ((position.y - PADDING) / CELL_HEIGHT).floor().max(0.0);
    (column as usize, line as usize)
}

fn origin(column: usize, line: usize) -> Point {
    Point::new(
        PADDING + column as f32 * CELL_WIDTH,
        PADDING + line as f32 * CELL_HEIGHT,
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

fn draw_run(frame: &mut canvas::Frame, palette: &Palette, run: &Run) {
    let color = palette.color(run.foreground);
    let position = origin(run.column, run.line);
    let weight = if run.bold {
        font::Weight::Bold
    } else {
        font::Weight::Normal
    };
    frame.fill_text(canvas::Text {
        content: run.text.clone(),
        position,
        color,
        size: FONT_SIZE.into(),
        line_height: iced::widget::text::LineHeight::Absolute(CELL_HEIGHT.into()),
        font: Font { weight, ..FONT },
        ..canvas::Text::default()
    });
    if run.underline {
        let width = run.text.chars().count() as f32 * CELL_WIDTH;
        let start = Point::new(position.x, position.y + UNDERLINE_OFFSET);
        frame.fill_rectangle(start, Size::new(width, 1.0), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(bytes: &[u8]) -> Vec<Cell> {
        let mut screen = Screen::new(40, 4);
        screen.feed(bytes);
        screen.cells()
    }

    #[test]
    fn the_size_is_what_fits_inside_the_padding() {
        let size = size_for(Size::new(2.0 * PADDING + 90.5, 2.0 * PADDING + 45.0));

        assert_eq!((size.columns, size.lines), (10, 2));
    }

    #[test]
    fn a_tiny_view_still_has_a_usable_size() {
        let size = size_for(Size::new(3.0, 3.0));

        assert_eq!((size.columns, size.lines), (2, 1));
    }

    #[test]
    fn a_wheel_notch_up_scrolls_three_lines_back() {
        let up = mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 };
        let down = mouse::ScrollDelta::Pixels { x: 0.0, y: -40.0 };

        assert_eq!(wheel_lines(up), 3);
        assert_eq!(wheel_lines(down), -2);
    }

    #[test]
    fn a_position_maps_to_the_character_under_it() {
        let inside = Point::new(PADDING + 2.5 * CELL_WIDTH, PADDING + 1.2 * CELL_HEIGHT);

        assert_eq!(cell_at(inside), (2, 1));
        assert_eq!(cell_at(Point::new(-30.0, 1.0)), (0, 0));
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
