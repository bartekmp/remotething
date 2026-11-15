//! Searching the text of a terminal, scrollback included.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;

use crate::terminal::{Screen, SelectedSpan};

/// A match: a piece of one line of the screen or scrollback. Lines above the
/// newest screen have negative numbers. Columns are both included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub line: i32,
    pub start: usize,
    pub end: usize,
}

/// A match on a visible line, and whether it is the one the user is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoundSpan {
    pub span: SelectedSpan,
    pub current: bool,
}

impl Screen {
    /// Every place `needle` occurs, oldest line first, ignoring letter case.
    pub fn find(&self, needle: &str) -> Vec<Found> {
        let needle: Vec<char> = needle.chars().map(fold).collect();
        if needle.is_empty() {
            return Vec::new();
        }
        let history = self.term.grid().history_size() as i32;
        let last = self.term.screen_lines() as i32 - 1;
        let mut found = Vec::new();
        for line in -history..=last {
            let text = self.line_chars(line);
            let mut column = 0;
            while column + needle.len() <= text.len() {
                if text[column..column + needle.len()] == needle[..] {
                    found.push(Found {
                        line,
                        start: column,
                        end: column + needle.len() - 1,
                    });
                    column += needle.len();
                } else {
                    column += 1;
                }
            }
        }
        found
    }

    /// The characters of a line, one per column, in folded case.
    fn line_chars(&self, line: i32) -> Vec<char> {
        let row = &self.term.grid()[Line(line)];
        (0..self.term.columns())
            .map(|column| {
                let cell = &row[Column(column)];
                // The second half of a wide character holds nothing to match.
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    ' '
                } else {
                    fold(cell.c)
                }
            })
            .collect()
    }

    /// Remembers the matches to highlight, and which one is the current one.
    pub fn set_found(&mut self, found: Vec<Found>, current: Option<usize>) {
        self.found = found;
        self.current_found = current;
    }

    /// Drops the highlights, for when the text moved under them.
    pub fn clear_found(&mut self) {
        self.found.clear();
        self.current_found = None;
    }

    /// Scrolls so the match is in view, about the middle of the screen.
    pub fn show_found(&mut self, found: Found) {
        let lines = self.lines() as i32;
        let wanted = (lines / 2 - found.line).clamp(0, self.term.grid().history_size() as i32);
        let delta = wanted - self.scrolled_back() as i32;
        self.scroll(delta);
    }

    /// The remembered matches that are on screen now.
    pub fn found_spans(&self) -> Vec<FoundSpan> {
        let scrolled_back = self.scrolled_back() as i32;
        let lines = self.lines() as i32;
        let mut spans = Vec::new();
        for (index, found) in self.found.iter().enumerate() {
            let line = found.line + scrolled_back;
            if (0..lines).contains(&line) {
                spans.push(FoundSpan {
                    span: SelectedSpan {
                        line: line as usize,
                        start: found.start,
                        end: found.end,
                    },
                    current: self.current_found == Some(index),
                });
            }
        }
        spans
    }
}

/// The form of a character used to compare ignoring case.
fn fold(character: char) -> char {
    character.to_lowercase().next().unwrap_or(character)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(text: &str) -> Screen {
        let mut screen = Screen::new(20, 3);
        screen.feed(text.as_bytes());
        screen
    }

    #[test]
    fn every_occurrence_is_found_ignoring_case() {
        let screen = screen("Hello hello\r\nHELLO");

        let found = screen.find("hello");

        let places: Vec<(i32, usize)> = found.iter().map(|f| (f.line, f.start)).collect();
        assert_eq!(places, [(0, 0), (0, 6), (1, 0)]);
        assert_eq!(found[0].end, 4);
    }

    #[test]
    fn nothing_matches_an_empty_or_absent_needle() {
        let screen = screen("hello");

        assert!(screen.find("").is_empty());
        assert!(screen.find("xyz").is_empty());
    }

    #[test]
    fn text_that_scrolled_off_the_screen_is_still_searched() {
        let mut screen = Screen::new(20, 2);
        screen.feed(b"first\r\nsecond\r\nthird\r\nfourth");

        let found = screen.find("first");

        assert_eq!(found.len(), 1);
        assert!(found[0].line < 0);
    }

    #[test]
    fn showing_a_match_in_the_scrollback_scrolls_to_it() {
        let mut screen = Screen::new(20, 4);
        screen.feed(b"needle\r\na\r\nb\r\nc\r\nd\r\ne\r\nf");
        let found = screen.find("needle")[0];
        assert!(screen.found_spans().is_empty());
        screen.set_found(vec![found], Some(0));

        screen.show_found(found);

        let spans = screen.found_spans();
        assert_eq!(spans.len(), 1);
        assert!(spans[0].current);
    }
}
