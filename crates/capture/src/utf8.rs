//! Turns a stream of bytes into text without cutting characters in half.

/// Collects bytes and hands out the text they spell, keeping back the start of
/// a character that has not fully arrived yet.
#[derive(Debug, Default)]
pub struct Utf8Splitter {
    pending: Vec<u8>,
}

impl Utf8Splitter {
    /// A splitter with nothing waiting.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `bytes` and returns the text that is complete. Bytes that are not
    /// valid text become the replacement character, so the output is always
    /// valid text.
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut text = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(valid) => {
                    text.push_str(valid);
                    self.pending.clear();
                    return text;
                }
                Err(error) => {
                    let good = error.valid_up_to();
                    // The unwrap cannot fail: those bytes were just checked.
                    text.push_str(std::str::from_utf8(&self.pending[..good]).unwrap_or_default());
                    match error.error_len() {
                        // The bytes are wrong, not just unfinished.
                        Some(bad) => {
                            text.push(char::REPLACEMENT_CHARACTER);
                            self.pending.drain(..good + bad);
                        }
                        // The end is the start of a character that has not arrived.
                        None => {
                            self.pending.drain(..good);
                            return text;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_passes_through() {
        assert_eq!(Utf8Splitter::new().push(b"hello"), "hello");
    }

    #[test]
    fn a_character_split_across_two_pushes_is_joined() {
        let bytes = "zażółć".as_bytes();
        let mut splitter = Utf8Splitter::new();

        // Cut inside the two bytes of `ż`.
        let first = splitter.push(&bytes[..3]);
        let second = splitter.push(&bytes[3..]);

        assert_eq!(format!("{first}{second}"), "zażółć");
        assert_eq!(first, "za");
    }

    #[test]
    fn bytes_that_are_not_text_become_the_replacement_character() {
        let mut splitter = Utf8Splitter::new();

        assert_eq!(splitter.push(&[b'a', 0xff, b'b']), "a\u{fffd}b");
    }

    #[test]
    fn nothing_is_left_waiting_after_complete_text() {
        let mut splitter = Utf8Splitter::new();
        splitter.push("ż".as_bytes());

        assert_eq!(splitter.push(b"x"), "x");
    }
}
