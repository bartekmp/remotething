//! The extra columns of the file browser: permissions and modification time as text.

use crate::capture_files::civil_date;

/// Permission bits written the way `ls` does: `0o754` is `rwxr-xr--`.
pub fn permissions_text(bits: u32) -> String {
    let mut text = String::with_capacity(9);
    for shift in [6, 3, 0] {
        let group = (bits >> shift) & 0o7;
        text.push(if group & 0o4 != 0 { 'r' } else { '-' });
        text.push(if group & 0o2 != 0 { 'w' } else { '-' });
        text.push(if group & 0o1 != 0 { 'x' } else { '-' });
    }
    text
}

/// A modification time, in seconds since 1970, as `2024-02-29 13:05` in UTC.
pub fn modified_text(unix: u64) -> String {
    let (year, month, day) = civil_date(unix / 86_400);
    let seconds = unix % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60
    )
}

#[cfg(test)]
#[path = "file_details_tests.rs"]
mod tests;
