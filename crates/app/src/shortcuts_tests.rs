//! Tests of `shortcuts.rs`.

use super::*;

#[test]
fn every_line_has_keys_and_a_description() {
    let lines = lines();

    assert!(!lines.is_empty());
    for (keys, what) in lines {
        assert!(!keys.is_empty());
        assert!(
            !what.is_empty() && !what.starts_with("shortcuts-"),
            "{what}"
        );
    }
}
