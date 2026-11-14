use zeroize::Zeroizing;

/// A password or passphrase held in memory.
///
/// It deliberately has no `Debug` or `Display`, so it cannot end up in a log
/// line or an error message by accident, and its memory is wiped on drop.
#[derive(Clone)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    /// Takes ownership of the text; the caller's copy is moved, not duplicated.
    pub fn new(text: String) -> Self {
        Self(Zeroizing::new(text))
    }

    /// The secret text. Every call site is a place where it could leak, so keep them few.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expose_returns_the_original_text() {
        let secret = Secret::new("hunter2".to_string());

        assert_eq!(secret.expose(), "hunter2");
    }

    #[test]
    fn a_clone_holds_the_same_text() {
        let secret = Secret::new("hunter2".to_string());

        assert_eq!(secret.clone().expose(), "hunter2");
    }
}
