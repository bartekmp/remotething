//! The terminal's screen: a grid of characters fed by bytes from the remote side.
//! It knows nothing about drawing or about where the bytes come from.

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Processor};

/// A colour as the remote program asked for it; the view turns it into pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    /// The terminal's default text colour.
    Foreground,
    /// The terminal's default background.
    Background,
    /// One of the 16 theme colours: 0 to 7 normal, 8 to 15 bright.
    Ansi(u8),
    Rgb(u8, u8, u8),
}

/// One character on screen with its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub column: usize,
    pub line: usize,
    pub character: char,
    pub foreground: Paint,
    pub background: Paint,
    pub bold: bool,
    pub underline: bool,
}

struct GridSize {
    columns: usize,
    lines: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.lines
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

/// Collects what the terminal wants to tell the remote side, such as a cursor position report.
#[derive(Clone, Default)]
struct Replies(Arc<Mutex<Vec<u8>>>);

impl EventListener for Replies {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event
            && let Ok(mut replies) = self.0.lock()
        {
            replies.extend_from_slice(text.as_bytes());
        }
    }
}

/// The selected part of one visible line: from `start` to `end`, both included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedSpan {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

pub struct Screen {
    term: Term<Replies>,
    parser: Processor,
    replies: Replies,
    /// Where the mouse button went down for the selection being made.
    anchor: Option<Point>,
    /// Whether the selection grows by characters, words or lines.
    granularity: Granularity,
}

/// The unit a selection grows by: a click selects characters, a double click
/// words and a triple click whole lines.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Granularity {
    #[default]
    Character,
    Word,
    Line,
}

impl Granularity {
    fn selection_type(self) -> SelectionType {
        match self {
            Granularity::Character => SelectionType::Simple,
            Granularity::Word => SelectionType::Semantic,
            Granularity::Line => SelectionType::Lines,
        }
    }
}

impl Screen {
    pub fn new(columns: usize, lines: usize) -> Self {
        let replies = Replies::default();
        let size = GridSize { columns, lines };
        Self {
            term: Term::new(Config::default(), &size, replies.clone()),
            parser: Processor::new(),
            replies,
            anchor: None,
            granularity: Granularity::default(),
        }
    }

    /// Applies bytes from the remote side and returns what must be sent back to it.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.parser.advance(&mut self.term, bytes);
        match self.replies.0.lock() {
            Ok(mut replies) => std::mem::take(&mut *replies),
            Err(_) => Vec::new(),
        }
    }

    pub fn resize(&mut self, columns: usize, lines: usize) {
        self.term.resize(GridSize { columns, lines });
    }

    pub fn columns(&self) -> usize {
        self.term.columns()
    }

    pub fn lines(&self) -> usize {
        self.term.screen_lines()
    }

    /// Moves the view into the lines that scrolled off the top; negative goes back down.
    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }

    /// Returns the view to the newest lines.
    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    /// How many lines the view is scrolled back.
    pub fn scrolled_back(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// A visible position as a place in the scrollback, where selections live.
    fn grid_point(&self, column: usize, line: usize) -> Point {
        let column = column.min(self.columns().saturating_sub(1));
        let line = line.min(self.lines().saturating_sub(1)) as i32;
        Point::new(Line(line - self.scrolled_back() as i32), Column(column))
    }

    /// Starts a selection at a visible position, dropping any earlier one.
    ///
    /// A word or a line is selected at once, and dragging grows it by whole
    /// words or lines.
    pub fn start_selection_by(&mut self, column: usize, line: usize, granularity: Granularity) {
        self.anchor = Some(self.grid_point(column, line));
        self.granularity = granularity;
        self.term.selection = None;
        if granularity != Granularity::Character {
            self.extend_selection(column, line);
        }
    }

    /// Stretches the selection from where it started to a visible position.
    pub fn extend_selection(&mut self, column: usize, line: usize) {
        let Some(anchor) = self.anchor else {
            return;
        };
        let point = self.grid_point(column, line);
        // Both end characters belong to the selection, whichever way it is dragged.
        let (anchor_side, point_side) = if point >= anchor {
            (Side::Left, Side::Right)
        } else {
            (Side::Right, Side::Left)
        };
        let mut selection = Selection::new(self.granularity.selection_type(), anchor, anchor_side);
        selection.update(point, point_side);
        self.term.selection = Some(selection);
    }

    /// The selected text, with a line break where the selection crosses lines.
    pub fn selected_text(&self) -> Option<String> {
        self.term
            .selection_to_string()
            .filter(|text| !text.is_empty())
    }

    /// The selection as the parts of the visible lines to highlight.
    pub fn selected_spans(&self) -> Vec<SelectedSpan> {
        let Some(range) = self.term.renderable_content().selection else {
            return Vec::new();
        };
        let scrolled_back = self.scrolled_back() as i32;
        let last_column = self.columns().saturating_sub(1);

        let mut spans = Vec::new();
        for line in 0..self.lines() {
            let grid_line = Line(line as i32 - scrolled_back);
            if grid_line < range.start.line || grid_line > range.end.line {
                continue;
            }
            let start = if grid_line == range.start.line {
                range.start.column.0
            } else {
                0
            };
            let end = if grid_line == range.end.line {
                range.end.column.0
            } else {
                last_column
            };
            spans.push(SelectedSpan { line, start, end });
        }
        spans
    }

    /// Whether the remote program asked for pastes to be marked as such.
    pub fn bracketed_paste(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Whether arrow keys must be sent in the form full-screen programs ask for.
    pub fn application_cursor(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    /// Where the cursor is, as column and line, unless the program hid it.
    pub fn cursor(&self) -> Option<(usize, usize)> {
        let cursor = self.term.renderable_content().cursor;
        // The cursor sits on the newest lines, which are out of view while scrolled back.
        if cursor.shape == CursorShape::Hidden || self.scrolled_back() > 0 {
            return None;
        }
        let line = usize::try_from(cursor.point.line.0).ok()?;
        Some((cursor.point.column.0, line))
    }

    /// Every cell that shows something: a character, or a background of its own.
    pub fn cells(&self) -> Vec<Cell> {
        let mut cells = Vec::new();
        // Lines above the newest screen have negative numbers; shift them into view.
        let scrolled_back = self.scrolled_back() as i32;
        for indexed in self.term.renderable_content().display_iter {
            let flags = indexed.cell.flags;
            // The second half of a double-width character is drawn with the first.
            if flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let Ok(line) = usize::try_from(indexed.point.line.0 + scrolled_back) else {
                continue;
            };

            let mut foreground = paint(indexed.cell.fg);
            let mut background = paint(indexed.cell.bg);
            if flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut foreground, &mut background);
            }
            let character = if flags.contains(Flags::HIDDEN) {
                ' '
            } else {
                indexed.cell.c
            };
            if character == ' ' && background == Paint::Background {
                continue;
            }

            cells.push(Cell {
                column: indexed.point.column.0,
                line,
                character,
                foreground,
                background,
                bold: flags.contains(Flags::BOLD),
                underline: flags.intersects(Flags::ALL_UNDERLINES),
            });
        }
        cells
    }

    /// The visible screen as plain text, one line per row.
    #[cfg(test)]
    pub fn text(&self) -> String {
        let last_line = self.term.screen_lines() as i32 - 1;
        let last_column = self.term.columns() - 1;
        let start = Point::new(Line(0), Column(0));
        let end = Point::new(Line(last_line), Column(last_column));
        self.term.bounds_to_string(start, end)
    }
}

fn paint(color: Color) -> Paint {
    match color {
        Color::Spec(rgb) => Paint::Rgb(rgb.r, rgb.g, rgb.b),
        Color::Indexed(index) => indexed(index),
        Color::Named(name) => named(name),
    }
}

fn named(name: NamedColor) -> Paint {
    match name {
        NamedColor::Background => Paint::Background,
        NamedColor::Black | NamedColor::DimBlack => Paint::Ansi(0),
        NamedColor::Red | NamedColor::DimRed => Paint::Ansi(1),
        NamedColor::Green | NamedColor::DimGreen => Paint::Ansi(2),
        NamedColor::Yellow | NamedColor::DimYellow => Paint::Ansi(3),
        NamedColor::Blue | NamedColor::DimBlue => Paint::Ansi(4),
        NamedColor::Magenta | NamedColor::DimMagenta => Paint::Ansi(5),
        NamedColor::Cyan | NamedColor::DimCyan => Paint::Ansi(6),
        NamedColor::White | NamedColor::DimWhite => Paint::Ansi(7),
        NamedColor::BrightBlack => Paint::Ansi(8),
        NamedColor::BrightRed => Paint::Ansi(9),
        NamedColor::BrightGreen => Paint::Ansi(10),
        NamedColor::BrightYellow => Paint::Ansi(11),
        NamedColor::BrightBlue => Paint::Ansi(12),
        NamedColor::BrightMagenta => Paint::Ansi(13),
        NamedColor::BrightCyan => Paint::Ansi(14),
        NamedColor::BrightWhite => Paint::Ansi(15),
        // Foreground, its bright and dim forms, and the cursor colour.
        _ => Paint::Foreground,
    }
}

/// The 256-colour table: 16 theme colours, a 6x6x6 colour cube, 24 greys.
fn indexed(index: u8) -> Paint {
    const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        0..=15 => Paint::Ansi(index),
        16..=231 => {
            let position = usize::from(index - 16);
            Paint::Rgb(
                CUBE_LEVELS[position / 36],
                CUBE_LEVELS[(position / 6) % 6],
                CUBE_LEVELS[position % 6],
            )
        }
        232..=255 => {
            let grey = 8 + 10 * (index - 232);
            Paint::Rgb(grey, grey, grey)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell_at(screen: &Screen, column: usize, line: usize) -> Cell {
        let cells = screen.cells();
        *cells
            .iter()
            .find(|cell| cell.column == column && cell.line == line)
            .expect("a visible cell")
    }

    #[test]
    fn plain_text_lands_on_the_first_row() {
        let mut screen = Screen::new(20, 4);

        screen.feed(b"hello");

        assert_eq!(screen.text().lines().next(), Some("hello"));
        assert_eq!(screen.cursor(), Some((5, 0)));
    }

    #[test]
    fn a_sequence_split_across_two_reads_still_parses() {
        let mut screen = Screen::new(20, 4);

        screen.feed(b"\x1b[3");
        screen.feed(b"1mred");

        assert_eq!(screen.text().lines().next(), Some("red"));
        assert_eq!(cell_at(&screen, 0, 0).foreground, Paint::Ansi(1));
    }

    #[test]
    fn blank_cells_are_left_out() {
        let mut screen = Screen::new(20, 4);

        screen.feed(b"a b");

        let columns: Vec<usize> = screen.cells().iter().map(|cell| cell.column).collect();
        assert_eq!(columns, [0, 2]);
    }

    #[test]
    fn bold_underline_and_true_colour_are_kept() {
        let mut screen = Screen::new(20, 4);

        screen.feed(b"\x1b[1;4;38;2;10;20;30mx");

        let cell = cell_at(&screen, 0, 0);
        assert!(cell.bold);
        assert!(cell.underline);
        assert_eq!(cell.foreground, Paint::Rgb(10, 20, 30));
    }

    #[test]
    fn inverse_video_swaps_the_two_colours() {
        let mut screen = Screen::new(20, 4);

        screen.feed(b"\x1b[7m ");

        let cell = cell_at(&screen, 0, 0);
        assert_eq!(cell.background, Paint::Foreground);
        assert_eq!(cell.foreground, Paint::Background);
    }

    #[test]
    fn the_256_colour_table_has_cube_and_grey_ramp() {
        assert_eq!(indexed(9), Paint::Ansi(9));
        assert_eq!(indexed(16), Paint::Rgb(0, 0, 0));
        assert_eq!(indexed(196), Paint::Rgb(255, 0, 0));
        assert_eq!(indexed(231), Paint::Rgb(255, 255, 255));
        assert_eq!(indexed(232), Paint::Rgb(8, 8, 8));
        assert_eq!(indexed(255), Paint::Rgb(238, 238, 238));
    }

    #[test]
    fn a_cursor_position_request_is_answered() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"abc");

        let reply = screen.feed(b"\x1b[6n");

        assert_eq!(reply, b"\x1b[1;4R");
    }

    #[test]
    fn a_hidden_cursor_is_not_reported() {
        let mut screen = Screen::new(20, 4);

        screen.feed(b"\x1b[?25l");

        assert_eq!(screen.cursor(), None);
    }

    #[test]
    fn scrolling_back_shows_lines_that_left_the_screen() {
        let mut screen = Screen::new(20, 2);
        screen.feed(b"one\r\ntwo\r\nthree");
        assert_eq!(cell_at(&screen, 0, 0).character, 't');

        screen.scroll(1);

        let first_line: String = screen
            .cells()
            .iter()
            .filter(|cell| cell.line == 0)
            .map(|cell| cell.character)
            .collect();
        assert_eq!(first_line, "one");
        assert_eq!(screen.cursor(), None);

        screen.scroll_to_bottom();
        assert_eq!(screen.scrolled_back(), 0);
        assert!(screen.cursor().is_some());
    }

    #[test]
    fn scrolling_stops_at_the_oldest_line() {
        let mut screen = Screen::new(20, 2);
        screen.feed(b"one\r\ntwo\r\nthree");

        screen.scroll(50);

        assert_eq!(screen.scrolled_back(), 1);
    }

    #[test]
    fn a_double_click_selects_the_word_and_a_triple_click_the_line() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"hello world");

        screen.start_selection_by(8, 0, Granularity::Word);
        assert_eq!(screen.selected_text().as_deref(), Some("world"));

        screen.start_selection_by(2, 0, Granularity::Line);
        // A selected line ends with its line break, like in other terminals.
        assert_eq!(screen.selected_text().as_deref(), Some("hello world\n"));
    }

    #[test]
    fn dragging_after_a_double_click_grows_by_words() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"hello big world");

        screen.start_selection_by(1, 0, Granularity::Word);
        screen.extend_selection(7, 0);

        assert_eq!(screen.selected_text().as_deref(), Some("hello big"));
    }

    #[test]
    fn a_drag_selects_the_characters_between_both_ends() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"hello world");

        screen.start_selection_by(6, 0, Granularity::Character);
        screen.extend_selection(10, 0);

        assert_eq!(screen.selected_text().as_deref(), Some("world"));
        let span = SelectedSpan {
            line: 0,
            start: 6,
            end: 10,
        };
        assert_eq!(screen.selected_spans(), [span]);
    }

    #[test]
    fn dragging_backwards_selects_the_same_text() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"hello world");

        screen.start_selection_by(10, 0, Granularity::Character);
        screen.extend_selection(6, 0);

        assert_eq!(screen.selected_text().as_deref(), Some("world"));
    }

    #[test]
    fn a_selection_over_two_lines_has_a_line_break_and_two_spans() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"one\r\ntwo");

        screen.start_selection_by(1, 0, Granularity::Character);
        screen.extend_selection(1, 1);

        assert_eq!(screen.selected_text().as_deref(), Some("ne\ntw"));
        let spans = screen.selected_spans();
        assert_eq!((spans[0].start, spans[0].end), (1, 19));
        assert_eq!((spans[1].start, spans[1].end), (0, 1));
    }

    #[test]
    fn a_click_without_a_drag_selects_nothing() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"hello");
        screen.start_selection_by(0, 0, Granularity::Character);
        screen.extend_selection(4, 0);

        screen.start_selection_by(2, 0, Granularity::Character);

        assert_eq!(screen.selected_text(), None);
        assert!(screen.selected_spans().is_empty());
    }

    #[test]
    fn a_selection_stays_on_its_text_when_the_view_scrolls() {
        let mut screen = Screen::new(20, 2);
        screen.feed(b"one\r\ntwo\r\nthree");
        screen.start_selection_by(0, 1, Granularity::Character);
        screen.extend_selection(4, 1);
        assert_eq!(screen.selected_text().as_deref(), Some("three"));

        screen.scroll(1);

        // "three" is now below the view; "two" took its line and is not selected.
        assert!(screen.selected_spans().is_empty());
        assert_eq!(screen.selected_text().as_deref(), Some("three"));
    }

    #[test]
    fn resizing_changes_the_grid() {
        let mut screen = Screen::new(20, 4);

        screen.resize(100, 30);

        assert_eq!((screen.columns(), screen.lines()), (100, 30));
    }

    #[test]
    fn full_screen_programs_can_switch_the_arrow_key_mode() {
        let mut screen = Screen::new(20, 4);
        assert!(!screen.application_cursor());

        screen.feed(b"\x1b[?1h");

        assert!(screen.application_cursor());
    }
}
