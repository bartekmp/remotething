//! Tests of `file_details.rs`.

use super::*;

#[test]
fn permissions_read_like_ls() {
    assert_eq!(permissions_text(0o754), "rwxr-xr--");
    assert_eq!(permissions_text(0o644), "rw-r--r--");
    assert_eq!(permissions_text(0), "---------");
    // The special bits above the nine are not shown.
    assert_eq!(permissions_text(0o4755), "rwxr-xr-x");
}

#[test]
fn a_time_shows_date_and_minutes_in_utc() {
    assert_eq!(modified_text(0), "1970-01-01 00:00");
    // 2024-02-29 13:05:59
    assert_eq!(
        modified_text(19_782 * 86_400 + 13 * 3600 + 5 * 60 + 59),
        "2024-02-29 13:05"
    );
}
