//! What the window does when its own size changes.

use iced::{Size, Task};

use crate::app::{Message, State};

/// How long the size of the window stays on screen after the last resize.
const SIZE_SHOWN_FOR: std::time::Duration = std::time::Duration::from_millis(900);

/// The window got a new size: show it for a moment.
pub(crate) fn on_resized(state: &mut State, size: Size) -> Task<Message> {
    if size == state.window {
        return Task::none();
    }
    state.window = size;
    state.size_shown = true;
    state.tabs.size_badge = true;
    state.resize_count += 1;
    let count = state.resize_count;
    Task::perform(
        async { tokio::time::sleep(SIZE_SHOWN_FOR).await },
        move |()| Message::ResizeSettled(count),
    )
}

/// The time to show the size is up. If no other resize came since, hide it and
/// ask whether the window is maximised, to remember that.
pub(crate) fn on_resize_settled(state: &mut State, count: u64) -> Task<Message> {
    if count != state.resize_count {
        return Task::none();
    }
    state.size_shown = false;
    state.tabs.size_badge = false;
    iced::window::oldest()
        .and_then(iced::window::is_maximized)
        .map(Message::WindowMaximized)
}
