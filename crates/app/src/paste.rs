//! What happens to text between the clipboard and a remote shell.

/// A paste longer than this asks first, even on one line.
const LONG_PASTE: usize = 2000;
/// How many lines of a paste the question shows.
const PREVIEW_LINES: usize = 5;
/// How much of each previewed line is shown.
const PREVIEW_WIDTH: usize = 80;

const BRACKET_START: &str = "\x1b[200~";
const BRACKET_END: &str = "\x1b[201~";

/// Whether a paste should be confirmed: a line break would run what precedes
/// it as a command, a very long paste is rarely meant, and control characters
/// have no business in copied text.
pub fn needs_confirmation(text: &str) -> bool {
    text.chars()
        .any(|character| character.is_control() && character != '\t')
        || text.chars().count() > LONG_PASTE
}

/// How many lines a paste has. A trailing line break does not start another line.
pub fn line_count(text: &str) -> usize {
    text.lines().count().max(1)
}

/// The first lines of a paste, shortened, for showing in the question.
pub fn preview(text: &str) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .take(PREVIEW_LINES)
        .map(|line| {
            if line.chars().count() > PREVIEW_WIDTH {
                let start: String = line.chars().take(PREVIEW_WIDTH).collect();
                format!("{start}…")
            } else {
                line.to_string()
            }
        })
        .collect();
    if line_count(text) > PREVIEW_LINES {
        lines.push("…".to_string());
    }
    lines.join("\n")
}

/// The bytes to send for a paste.
///
/// Line breaks become carriage returns, as if Enter was pressed. With
/// `bracketed` (the remote program asked for it) the text is wrapped in
/// markers so the program can tell a paste from typing.
///
/// Every control character except tab and line breaks is dropped. Without an
/// Escape the text cannot contain an end marker, or build one, and so cannot
/// make the rest of itself pose as typing.
pub fn to_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let text: String = text
        .replace("\r\n", "\r")
        .chars()
        .map(|character| if character == '\n' { '\r' } else { character })
        .filter(|character| !character.is_control() || matches!(character, '\t' | '\r'))
        .collect();
    if !bracketed {
        return text.into_bytes();
    }
    format!("{BRACKET_START}{text}{BRACKET_END}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_single_line_is_pasted_without_asking() {
        assert!(!needs_confirmation("ls -l /var/log"));
    }

    #[test]
    fn any_line_break_asks_first() {
        assert!(needs_confirmation("ls\n"));
        assert!(needs_confirmation("one\r\ntwo"));
    }

    #[test]
    fn a_very_long_line_asks_first() {
        assert!(needs_confirmation(&"x".repeat(LONG_PASTE + 1)));
        assert!(!needs_confirmation(&"x".repeat(LONG_PASTE)));
    }

    #[test]
    fn lines_are_counted_without_the_trailing_break() {
        assert_eq!(line_count("one"), 1);
        assert_eq!(line_count("one\ntwo\n"), 2);
        assert_eq!(line_count(""), 1);
    }

    #[test]
    fn the_preview_shows_the_first_lines_and_marks_what_is_cut() {
        let text = "1\n2\n3\n4\n5\n6\n7";

        assert_eq!(preview(text), "1\n2\n3\n4\n5\n…");
        assert_eq!(preview("short"), "short");
        assert!(preview(&"x".repeat(200)).ends_with('…'));
    }

    #[test]
    fn line_breaks_are_sent_as_enter() {
        assert_eq!(to_bytes("a\nb\r\nc", false), b"a\rb\rc");
    }

    #[test]
    fn a_bracketed_paste_is_wrapped_and_cannot_end_itself_early() {
        assert_eq!(to_bytes("ls", true), b"\x1b[200~ls\x1b[201~");

        // The second text only contains an end marker once the first one is removed.
        for sneaky in [
            "safe\x1b[201~rm -rf /\n",
            "safe\x1b[20\x1b[201~1~rm -rf /\n",
        ] {
            let sent = String::from_utf8(to_bytes(sneaky, true)).unwrap();
            assert_eq!(sent.matches(BRACKET_END).count(), 1, "{sneaky:?}");
            assert!(sent.ends_with(BRACKET_END));
            assert_eq!(sent.matches('\x1b').count(), 2, "{sneaky:?}");
        }
    }

    #[test]
    fn control_characters_are_dropped_and_ask_first() {
        let text = "ls\x1b[2J\x03 -l\tx";

        assert!(needs_confirmation(text));
        assert_eq!(to_bytes(text, false), b"ls[2J -l\tx");
        assert!(!needs_confirmation("a\tb"));
    }
}
