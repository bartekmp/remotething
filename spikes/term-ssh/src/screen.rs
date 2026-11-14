//! Terminal state fed by raw bytes. No PTY, no UI: the question behind risk R3.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;

struct Size {
    columns: usize,
    lines: usize,
}

impl Dimensions for Size {
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

/// A terminal grid that accepts bytes from any source.
pub struct Screen {
    term: Term<VoidListener>,
    parser: Processor,
}

impl Screen {
    pub fn new(columns: usize, lines: usize) -> Self {
        let size = Size { columns, lines };
        let term = Term::new(Config::default(), &size, VoidListener);
        Self {
            term,
            parser: Processor::new(),
        }
    }

    /// Apply bytes received from the remote side (escape sequences included).
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    /// The visible screen as plain text, one line per row.
    pub fn text(&self) -> String {
        let last_line = self.term.screen_lines() as i32 - 1;
        let last_column = self.term.columns() - 1;
        let start = Point::new(Line(0), Column(0));
        let end = Point::new(Line(last_line), Column(last_column));
        self.term.bounds_to_string(start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_lands_on_the_first_row() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"hello");
        assert_eq!(screen.text().lines().next(), Some("hello"));
    }

    #[test]
    fn cursor_movement_sequences_are_interpreted() {
        let mut screen = Screen::new(20, 4);
        // Move to row 2, column 3, then print.
        screen.feed(b"\x1b[2;3Hxy");
        assert_eq!(screen.text().lines().nth(1), Some("  xy"));
    }

    #[test]
    fn colour_sequences_do_not_leak_into_text() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"\x1b[31mred\x1b[0m");
        assert_eq!(screen.text().lines().next(), Some("red"));
    }

    #[test]
    fn a_sequence_split_across_two_reads_still_parses() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"\x1b[3");
        screen.feed(b"1mred");
        assert_eq!(screen.text().lines().next(), Some("red"));
    }
}
