//! Where tabs go in the bar: how many fit on a line, and which are left out.

use std::ops::Range;

pub const TAB_WIDTH: f32 = 170.0;
pub const TAB_HEIGHT: f32 = 34.0;
pub const GAP: f32 = 6.0;
/// The "new tab" button and the "more tabs" button.
pub const BUTTON_WIDTH: f32 = 44.0;
/// A wrapped bar grows to this many lines and scrolls after that.
pub const MAX_LINES: usize = 3;

/// How many tabs fit side by side in `width`. Never less than one.
pub fn per_line(width: f32) -> usize {
    let fit = ((width + GAP) / (TAB_WIDTH + GAP)).floor();
    fit.max(1.0) as usize
}

/// The tabs shown on a single line: as many as fit, always including the active one.
///
/// `width` is the whole bar. Room for the two buttons is taken off when not every tab fits.
pub fn single_line(count: usize, active: usize, width: f32) -> Range<usize> {
    let with_new_button = width - BUTTON_WIDTH - GAP;
    if count <= per_line(with_new_button) {
        return 0..count;
    }
    let fit = per_line(with_new_button - BUTTON_WIDTH - GAP);
    // Slide the window just far enough to keep the active tab in it.
    let start = (active + 1).saturating_sub(fit);
    start..(start + fit).min(count)
}

/// How many lines a wrapped bar takes, and whether it has to scroll.
pub fn wrapped_lines(count: usize, width: f32) -> (usize, bool) {
    // The new-tab button counts as one more item at the end.
    let lines = (count + 1).div_ceil(per_line(width));
    (lines.clamp(1, MAX_LINES), lines > MAX_LINES)
}

/// The height of a bar of `lines` lines.
pub fn height(lines: usize) -> f32 {
    lines as f32 * TAB_HEIGHT + lines.saturating_sub(1) as f32 * GAP
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bar exactly wide enough for `tabs` tabs and the new-tab button.
    fn width_for(tabs: usize) -> f32 {
        tabs as f32 * (TAB_WIDTH + GAP) + BUTTON_WIDTH
    }

    #[test]
    fn a_narrow_bar_still_holds_one_tab() {
        assert_eq!(per_line(50.0), 1);
    }

    #[test]
    fn everything_is_shown_when_it_fits() {
        assert_eq!(single_line(4, 0, width_for(4)), 0..4);
    }

    #[test]
    fn too_many_tabs_leave_room_for_the_more_button() {
        // Five tabs in room for four: the more button costs part of a tab.
        assert_eq!(single_line(5, 0, width_for(4)), 0..3);
    }

    #[test]
    fn the_shown_tabs_slide_to_keep_the_active_one_visible() {
        assert_eq!(single_line(10, 6, width_for(4)), 4..7);
        assert_eq!(single_line(10, 9, width_for(4)), 7..10);
    }

    #[test]
    fn a_wrapped_bar_grows_to_three_lines_then_scrolls() {
        let width = width_for(4);

        assert_eq!(wrapped_lines(3, width), (1, false));
        assert_eq!(wrapped_lines(8, width), (3, false));
        assert_eq!(wrapped_lines(30, width), (3, true));
    }

    #[test]
    fn lines_are_separated_by_the_gap() {
        assert_eq!(height(1), TAB_HEIGHT);
        assert_eq!(height(3), 3.0 * TAB_HEIGHT + 2.0 * GAP);
    }
}
