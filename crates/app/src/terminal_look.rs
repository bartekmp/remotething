//! Putting the chosen terminal colours and text size to use.

use iced::Task;

use crate::app::{Message, State};
use crate::i18n::fl;
use crate::terminal_settings;
use crate::terminal_view;

impl State {
    /// Makes terminals use the chosen theme and text size, and redraws the open ones.
    pub(crate) fn apply_terminal_look(&mut self) {
        let palette = self.terminal_themes.applied_palette(&self.preferences);
        let terminal = &self.preferences.terminal;
        terminal_view::set_look(palette, terminal.font_size, terminal.font_family.as_deref());
        self.tabs.terminal_look_changed();
    }

    /// Handles a message of the Terminal page of the settings.
    pub(crate) fn terminal_message(
        &mut self,
        message: terminal_settings::Message,
    ) -> Task<Message> {
        let outcome = terminal_settings::update(
            &mut self.terminal_themes,
            &mut self.preferences,
            &self.path,
            message,
        );
        if outcome.preferences_changed {
            self.save_preferences();
        }
        self.apply_terminal_look();
        if outcome.pick_file {
            let dialog = crate::file_dialog::open_any(fl!("terminal-theme-import"));
            return Task::perform(dialog, |path| {
                Message::Settings(crate::settings::Message::Terminal(
                    terminal_settings::Message::FilePicked(path),
                ))
            });
        }
        Task::none()
    }
}
