//! Searching the terminal of a connected session: the bar above the terminal
//! and the state behind it. The matches themselves live in the screen.

use iced::Element;
use iced::widget::{button, row, text};

use crate::i18n::fl;
use crate::icons;
use crate::session::Message;
use crate::session_update::Live;
use crate::theme;

/// The widget id of the search field, so it can be focused when the bar opens.
pub const INPUT: &str = "terminal-search";

/// An open search bar.
#[derive(Debug, Default)]
pub struct Search {
    pub(crate) query: String,
    /// The match the user is on, an index into the screen's matches.
    pub(crate) current: usize,
}

impl Live {
    /// Shows the search bar.
    pub(crate) fn open_search(&mut self) {
        self.search.get_or_insert_with(Search::default);
    }

    /// Hides the bar and the highlights.
    pub(crate) fn close_search(&mut self) {
        self.search = None;
        self.screen.clear_found();
        self.cache.clear();
    }

    /// Searches for new text, going to the newest match.
    pub(crate) fn search_for(&mut self, query: String) {
        let Some(search) = &mut self.search else {
            return;
        };
        search.query = query;
        let found = self.screen.find(&search.query);
        search.current = found.len().saturating_sub(1);
        self.show_current(found);
    }

    /// Goes to the next or previous match, searching again first because
    /// the text may have changed since.
    pub(crate) fn search_step(&mut self, forward: bool) {
        let Some(search) = &mut self.search else {
            return;
        };
        let found = self.screen.find(&search.query);
        if found.is_empty() {
            self.show_current(found);
            return;
        }
        let count = found.len();
        search.current = if forward {
            (search.current + 1) % count
        } else {
            (search.current + count - 1) % count
        };
        self.show_current(found);
    }

    fn show_current(&mut self, found: Vec<crate::terminal_search::Found>) {
        let current = self.search.as_ref().map(|search| search.current);
        let target = current.and_then(|index| found.get(index).copied());
        self.screen.set_found(found, target.and(current));
        if let Some(target) = target {
            self.screen.show_found(target);
        }
        self.cache.clear();
    }
}

pub fn view<'a>(search: &'a Search, matches: usize) -> Element<'a, Message> {
    let field = theme::input(&fl!("search-hint"), &search.query)
        .id(INPUT)
        .on_input(Message::SearchChanged)
        .on_submit(Message::SearchNext);
    let count = if search.query.is_empty() {
        String::new()
    } else if matches == 0 {
        fl!("search-none")
    } else {
        let current: usize = search.current + 1;
        fl!("search-count", current = current, total = matches)
    };
    let step = |label: &'static str, message: Message| {
        button(text(label).size(theme::TEXT_SMALL))
            .style(theme::pill)
            .on_press(message)
    };
    row![
        field,
        text(count)
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        step("↑", Message::SearchPrevious),
        step("↓", Message::SearchNext),
        button(icons::view(icons::CLOSE, theme::TEXT_SMALL))
            .style(theme::pill)
            .on_press(Message::SearchClosed),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}
