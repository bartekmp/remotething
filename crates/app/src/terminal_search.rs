//! Searching the text of a terminal, scrollback included.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;

use crate::terminal::{Screen, SelectedSpan};

/// A match: from a character of one line to a character of the same or a later line,
/// when the text wrapped. Lines above the newest screen have negative numbers. Columns
/// are both included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub line: i32,
    pub start: usize,
    pub end_line: i32,
    pub end: usize,
}

/// A match on a visible line, and whether it is the one the user is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoundSpan {
    pub span: SelectedSpan,
    pub current: bool,
}

impl Screen {
    /// Every place `needle` occurs, oldest line first, ignoring letter case. Text the
    /// terminal wrapped onto the next line is searched as one line.
    pub fn find(&self, needle: &str) -> Vec<Found> {
        let needle: Vec<char> = needle.chars().map(fold).collect();
        if needle.is_empty() {
            return Vec::new();
        }
        let history = self.term.grid().history_size() as i32;
        let last = self.term.screen_lines() as i32 - 1;
        let mut found = Vec::new();
        // The characters of the text so far, with where each one is on screen.
        let mut text: Vec<(char, i32, usize)> = Vec::new();
        for line in -history..=last {
            let wraps = self.push_line(line, &mut text);
            if wraps && line < last {
                continue;
            }
            search_text(&text, &needle, &mut found);
            text.clear();
        }
        found
    }

    /// Adds the characters of a line to `text`, and tells whether the text goes on
    /// in the next line.
    fn push_line(&self, line: i32, text: &mut Vec<(char, i32, usize)>) -> bool {
        let row = &self.term.grid()[Line(line)];
        let columns = self.term.columns();
        for column in 0..columns {
            let cell = &row[Column(column)];
            // A gap left where a wide character did not fit holds nothing to match.
            if cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            // The second half of a wide character holds nothing to match.
            let character = if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                ' '
            } else {
                fold(cell.c)
            };
            text.push((character, line, column));
        }
        row[Column(columns - 1)].flags.contains(Flags::WRAPLINE)
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
            // A match that wrapped is highlighted on each of its lines.
            for line in found.line..=found.end_line {
                let shown = line + scrolled_back;
                if !(0..lines).contains(&shown) {
                    continue;
                }
                let start = if line == found.line { found.start } else { 0 };
                let end = if line == found.end_line {
                    found.end
                } else {
                    self.columns() - 1
                };
                spans.push(FoundSpan {
                    span: SelectedSpan {
                        line: shown as usize,
                        start,
                        end,
                    },
                    current: self.current_found == Some(index),
                });
            }
        }
        spans
    }
}

/// Adds to `found` each place `needle` occurs in the text, which keeps where each
/// character is on screen.
fn search_text(text: &[(char, i32, usize)], needle: &[char], found: &mut Vec<Found>) {
    let mut at = 0;
    while at + needle.len() <= text.len() {
        let same = text[at..at + needle.len()]
            .iter()
            .zip(needle)
            .all(|((character, _, _), wanted)| character == wanted);
        if !same {
            at += 1;
            continue;
        }
        let (_, line, start) = text[at];
        let (_, end_line, end) = text[at + needle.len() - 1];
        found.push(Found {
            line,
            start,
            end_line,
            end,
        });
        at += needle.len();
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
    fn text_that_wrapped_is_found_as_one_line() {
        // 20 columns: "wrapped" is split between the first and the second line.
        let screen = screen("0123456789012345678wrapped end");

        let found = screen.find("wrapped");

        assert_eq!(found.len(), 1);
        let found = found[0];
        assert_eq!((found.line, found.start), (0, 19));
        assert_eq!((found.end_line, found.end), (1, 5));
    }

    #[test]
    fn a_match_that_wrapped_is_highlighted_on_both_lines() {
        let mut screen = screen("0123456789012345678wrapped end");
        let found = screen.find("wrapped")[0];
        screen.set_found(vec![found], Some(0));

        let spans: Vec<(usize, usize, usize)> = screen
            .found_spans()
            .iter()
            .map(|found| (found.span.line, found.span.start, found.span.end))
            .collect();

        assert_eq!(spans, [(0, 19, 19), (1, 0, 5)]);
    }

    #[test]
    fn lines_the_program_ended_are_not_joined() {
        let screen = screen("0123456789012345678w\r\napped");

        assert!(screen.find("wapped").is_empty());
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
