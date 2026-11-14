//! The settings screen. Every change is saved at once; there is no Save button.

use std::fmt;

use iced::widget::{button, column, row, text};
use iced::{Element, Length};

use crate::i18n::{self, fl};
use crate::preferences::{Preferences, TabLayout, ThemeChoice};
use crate::theme;

const THEMES: [ThemeOption; 3] = [
    ThemeOption(ThemeChoice::System),
    ThemeOption(ThemeChoice::Light),
    ThemeOption(ThemeChoice::Dark),
];

const TAB_LAYOUTS: [TabLayoutOption; 2] = [
    TabLayoutOption(TabLayout::SingleLine),
    TabLayoutOption(TabLayout::Wrap),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabLayoutOption(TabLayout);

impl fmt::Display for TabLayoutOption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self.0 {
            TabLayout::SingleLine => fl!("settings-tabs-single-line"),
            TabLayout::Wrap => fl!("settings-tabs-wrap"),
        };
        formatter.write_str(&label)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    TabLayoutPicked(TabLayoutOption),
    ThemePicked(ThemeOption),
    LanguagePicked(LanguageOption),
    ExportPressed,
    ImportPressed,
    ClosePressed,
}

/// What the screen asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Changed,
    /// Ask where to save the connections, then write them.
    Export,
    /// Ask for an export file, then add what is in it.
    Import,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeOption(ThemeChoice);

impl fmt::Display for ThemeOption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self.0 {
            ThemeChoice::System => fl!("settings-system"),
            ThemeChoice::Light => fl!("theme-light"),
            ThemeChoice::Dark => fl!("theme-dark"),
        };
        formatter.write_str(&label)
    }
}

/// A language code, or `None` for "follow the system".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageOption(Option<String>);

impl fmt::Display for LanguageOption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            // Language codes are shown as they are, in every language.
            Some(code) => formatter.write_str(code),
            None => formatter.write_str(&fl!("settings-system")),
        }
    }
}

pub fn update(preferences: &mut Preferences, message: Message) -> Event {
    match message {
        Message::TabLayoutPicked(option) => {
            preferences.tab_layout = option.0;
            Event::Changed
        }
        Message::ThemePicked(option) => {
            preferences.theme = option.0;
            Event::Changed
        }
        Message::LanguagePicked(option) => {
            preferences.language = option.0;
            Event::Changed
        }
        Message::ExportPressed => Event::Export,
        Message::ImportPressed => Event::Import,
        Message::ClosePressed => Event::Close,
    }
}

pub fn view(preferences: &Preferences) -> Element<'_, Message> {
    let header = row![
        text(fl!("settings-title"))
            .size(theme::TEXT_DISPLAY)
            .width(Length::Fill),
        button(text(fl!("settings-close")))
            .style(theme::pill)
            .on_press(Message::ClosePressed),
    ];

    let theme = column![
        text(fl!("settings-theme"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        theme::pick(
            THEMES,
            Some(ThemeOption(preferences.theme)),
            Message::ThemePicked
        ),
    ]
    .spacing(2);

    let languages = language_options(i18n::available_languages());
    let selected = LanguageOption(preferences.language.clone());
    let language = column![
        text(fl!("settings-language"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        theme::pick(languages, Some(selected), Message::LanguagePicked),
        text(fl!("settings-language-note"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
    ]
    .spacing(2);

    let tabs = column![
        text(fl!("settings-tabs"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        theme::pick(
            TAB_LAYOUTS,
            Some(TabLayoutOption(preferences.tab_layout)),
            Message::TabLayoutPicked
        ),
    ]
    .spacing(2);

    let exchange = column![
        text(fl!("settings-exchange"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        row![
            button(text(fl!("settings-export")))
                .style(theme::pill)
                .on_press(Message::ExportPressed),
            button(text(fl!("settings-import")))
                .style(theme::pill)
                .on_press(Message::ImportPressed),
        ]
        .spacing(8),
        text(fl!("settings-exchange-note"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
    ]
    .spacing(4);

    column![header, theme, language, tabs, exchange]
        .spacing(16)
        .into()
}

fn language_options(codes: Vec<String>) -> Vec<LanguageOption> {
    let mut options = vec![LanguageOption(None)];
    options.extend(codes.into_iter().map(|code| LanguageOption(Some(code))));
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picking_a_theme_changes_the_preferences() {
        let mut preferences = Preferences::default();

        let event = update(
            &mut preferences,
            Message::ThemePicked(ThemeOption(ThemeChoice::Dark)),
        );

        assert_eq!(event, Event::Changed);
        assert_eq!(preferences.theme, ThemeChoice::Dark);
    }

    #[test]
    fn picking_a_language_and_then_the_system_again() {
        let mut preferences = Preferences::default();

        update(
            &mut preferences,
            Message::LanguagePicked(LanguageOption(Some("en".to_string()))),
        );
        assert_eq!(preferences.language.as_deref(), Some("en"));

        update(
            &mut preferences,
            Message::LanguagePicked(LanguageOption(None)),
        );
        assert_eq!(preferences.language, None);
    }

    #[test]
    fn close_changes_nothing() {
        let mut preferences = Preferences::default();

        assert_eq!(
            update(&mut preferences, Message::ClosePressed),
            Event::Close
        );
        assert_eq!(preferences, Preferences::default());
    }

    #[test]
    fn the_system_option_comes_first_then_every_shipped_language() {
        let options = language_options(vec!["en".to_string(), "pl".to_string()]);

        assert_eq!(options[0], LanguageOption(None));
        assert_eq!(options.len(), 3);
    }

    #[test]
    fn english_is_always_shipped() {
        assert!(i18n::available_languages().contains(&"en".to_string()));
    }
}
