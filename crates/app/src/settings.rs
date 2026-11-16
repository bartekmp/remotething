//! The settings screen. Every change is saved at once; there is no Save button.

use std::fmt;
use std::path::PathBuf;

use iced::widget::{button, checkbox, column, container, row, text};
use iced::{Element, Length};

use crate::branding;
use crate::features;
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

/// The pages of the settings screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    /// Theme, language and how the tab bar behaves.
    #[default]
    General,
    /// Moving connections to another copy of the app.
    ImportExport,
    /// The optional features and their folders.
    Features,
    /// The logo, name and version.
    About,
}

const TABS: [Tab; 4] = [Tab::General, Tab::ImportExport, Tab::Features, Tab::About];

impl Tab {
    fn label(self) -> String {
        match self {
            Tab::General => fl!("settings-tab-general"),
            Tab::ImportExport => fl!("settings-tab-exchange"),
            Tab::Features => fl!("settings-tab-features"),
            Tab::About => fl!("settings-tab-about"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    /// A page of the settings was picked.
    PagePicked(Tab),
    TabLayoutPicked(TabLayoutOption),
    ThemePicked(ThemeOption),
    LanguagePicked(LanguageOption),
    ScreenshotsToggled(bool),
    ConsoleRecordingToggled(bool),
    DesktopRecordingToggled(bool),
    KeystrokesToggled(bool),
    EveryRecordingToggled(bool),
    /// "Change…" on the folder of a feature.
    FolderChangePressed(FolderKind),
    /// "Default" on the folder of a feature.
    FolderResetPressed(FolderKind),
    ExportPressed,
    ImportPressed,
    ImportPuttyPressed,
    ClosePressed,
}

/// The folders the user can choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderKind {
    Screenshots,
    DesktopRecordings,
    ConsoleRecordings,
}

/// Sets or clears (with `None`) the folder of `kind`.
pub fn set_folder(preferences: &mut Preferences, kind: FolderKind, folder: Option<PathBuf>) {
    let folders = &mut preferences.folders;
    match kind {
        FolderKind::Screenshots => folders.screenshots = folder,
        FolderKind::DesktopRecordings => folders.desktop_recordings = folder,
        FolderKind::ConsoleRecordings => folders.console_recordings = folder,
    }
}

/// What the screen asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Changed,
    /// Another page was picked; the window remembers which.
    PageChanged(Tab),
    /// Ask which folder to use for this, then set it.
    PickFolder(FolderKind),
    /// Ask where to save the connections, then write them.
    Export,
    /// Ask for an export file, then add what is in it.
    Import,
    /// Open the screen that previews importing PuTTY sessions.
    ImportPutty,
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
        Message::PagePicked(tab) => Event::PageChanged(tab),
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
        Message::ScreenshotsToggled(on) => {
            preferences.features.screenshots = on;
            Event::Changed
        }
        Message::ConsoleRecordingToggled(on) => {
            preferences.features.console_recording = on;
            Event::Changed
        }
        Message::DesktopRecordingToggled(on) => {
            preferences.features.desktop_recording = on;
            Event::Changed
        }
        Message::KeystrokesToggled(on) => {
            preferences.features.record_keystrokes = on;
            Event::Changed
        }
        Message::EveryRecordingToggled(on) => {
            preferences.features.record_every_session = on;
            Event::Changed
        }
        Message::FolderChangePressed(kind) => Event::PickFolder(kind),
        Message::FolderResetPressed(kind) => {
            set_folder(preferences, kind, None);
            Event::Changed
        }
        Message::ExportPressed => Event::Export,
        Message::ImportPressed => Event::Import,
        Message::ImportPuttyPressed => Event::ImportPutty,
        Message::ClosePressed => Event::Close,
    }
}

pub fn view(preferences: &Preferences, page: Tab) -> Element<'_, Message> {
    let header = row![
        text(fl!("settings-title"))
            .size(theme::TEXT_DISPLAY)
            .width(Length::Fill),
        button(text(fl!("settings-close")))
            .style(theme::pill)
            .on_press(Message::ClosePressed),
    ];
    let content = match page {
        Tab::General => general(preferences),
        Tab::ImportExport => import_export(),
        Tab::Features => features(preferences),
        Tab::About => about(),
    };
    column![header, page_bar(page), content].spacing(16).into()
}

/// The row of buttons that picks the page.
fn page_bar<'a>(page: Tab) -> Element<'a, Message> {
    row(TABS.iter().map(|tab| {
        button(text(tab.label()))
            .padding([6, 12])
            .style(theme::entry(*tab == page))
            .on_press(Message::PagePicked(*tab))
            .into()
    }))
    .spacing(4)
    .wrap()
    .into()
}

/// Theme, language and the behaviour of the tab bar.
fn general(preferences: &Preferences) -> Element<'_, Message> {
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

    column![theme, language, tabs].spacing(16).into()
}

/// Exporting and importing the connections.
fn import_export<'a>() -> Element<'a, Message> {
    column![
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
            button(text(fl!("settings-import-putty")))
                .style(theme::pill)
                .on_press(Message::ImportPuttyPressed),
        ]
        .spacing(8)
        .wrap(),
        text(fl!("settings-exchange-note"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
    ]
    .spacing(4)
    .into()
}

/// The switches of the optional features. The ones that only matter with another one are shown below it.
fn features(preferences: &Preferences) -> Element<'_, Message> {
    let features = preferences.features;
    let mut switches = column![
        checkbox(features.screenshots)
            .label(fl!("settings-feature-screenshots"))
            .on_toggle(Message::ScreenshotsToggled),
    ]
    .spacing(6);
    if features.screenshots {
        switches = switches.push(folder_row(
            FolderKind::Screenshots,
            features::screenshots_folder(),
            preferences.folders.screenshots.is_some(),
        ));
    }
    switches = switches.push(
        checkbox(features.console_recording)
            .label(fl!("settings-feature-console"))
            .on_toggle(Message::ConsoleRecordingToggled),
    );
    if features.console_recording {
        switches = switches.push(folder_row(
            FolderKind::ConsoleRecordings,
            features::console_recordings_folder(),
            preferences.folders.console_recordings.is_some(),
        ));
    }
    switches = switches.push(
        checkbox(features.desktop_recording)
            .label(fl!("settings-feature-desktop"))
            .on_toggle(Message::DesktopRecordingToggled),
    );
    if features.desktop_recording {
        switches = switches.push(folder_row(
            FolderKind::DesktopRecordings,
            features::desktop_recordings_folder(),
            preferences.folders.desktop_recordings.is_some(),
        ));
    }
    if features.console_recording || features.desktop_recording {
        switches = switches.push(
            text(fl!("settings-features-note"))
                .size(theme::TEXT_SMALL)
                .style(theme::secondary_text),
        );
    }
    if features.console_recording {
        switches = switches
            .push(
                checkbox(features.record_every_session)
                    .label(fl!("settings-feature-every"))
                    .on_toggle(Message::EveryRecordingToggled),
            )
            .push(
                checkbox(features.record_keystrokes)
                    .label(fl!("settings-feature-keystrokes"))
                    .on_toggle(Message::KeystrokesToggled),
            );
    }
    switches.into()
}

/// Where a feature writes its files, with buttons to change it or go back to the default.
fn folder_row(kind: FolderKind, folder: PathBuf, chosen: bool) -> Element<'static, Message> {
    let pill = |label: String, message: Message| {
        button(text(label).size(theme::TEXT_SMALL))
            .style(theme::pill)
            .on_press(message)
    };
    let mut line = row![
        text(folder.display().to_string())
            .size(theme::TEXT_SMALL)
            .font(iced::Font::MONOSPACE)
            .style(theme::secondary_text)
            .wrapping(text::Wrapping::Glyph)
            .width(Length::Fill),
        pill(
            fl!("settings-folder-change"),
            Message::FolderChangePressed(kind)
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    if chosen {
        line = line.push(pill(
            fl!("settings-folder-default"),
            Message::FolderResetPressed(kind),
        ));
    }
    container(line)
        .padding(iced::Padding::default().left(26.0))
        .into()
}

/// The logo, name and version. The name and version stay unformatted and
/// untranslated, like `--version`: bug reports quote them.
fn about<'a>() -> Element<'a, Message> {
    let name = column![
        text("Remotething").size(theme::TEXT_TITLE),
        text(env!("CARGO_PKG_VERSION"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
        text(fl!("about-rights"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text),
    ]
    .spacing(2);
    row![branding::logo(56.0), name]
        .spacing(12)
        .align_y(iced::Alignment::Center)
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

    #[test]
    fn changing_a_folder_asks_the_window_to_pick_one() {
        let mut preferences = Preferences::default();

        let event = update(
            &mut preferences,
            Message::FolderChangePressed(FolderKind::ConsoleRecordings),
        );

        assert_eq!(event, Event::PickFolder(FolderKind::ConsoleRecordings));
    }

    #[test]
    fn a_picked_folder_is_kept_for_its_own_feature_and_the_default_button_clears_it() {
        let mut preferences = Preferences::default();

        set_folder(
            &mut preferences,
            FolderKind::Screenshots,
            Some(PathBuf::from("/data/shots")),
        );
        assert_eq!(
            preferences.folders.screenshots,
            Some(PathBuf::from("/data/shots"))
        );
        assert_eq!(preferences.folders.desktop_recordings, None);

        let event = update(
            &mut preferences,
            Message::FolderResetPressed(FolderKind::Screenshots),
        );
        assert_eq!(event, Event::Changed);
        assert!(preferences.folders.is_empty());
    }

    #[test]
    fn the_settings_open_on_the_general_page() {
        assert_eq!(Tab::default(), Tab::General);
    }

    #[test]
    fn picking_a_page_asks_the_window_to_remember_it_and_changes_nothing_saved() {
        let mut preferences = Preferences::default();
        let before = preferences.clone();

        let event = update(&mut preferences, Message::PagePicked(Tab::Features));

        assert_eq!(event, Event::PageChanged(Tab::Features));
        assert_eq!(preferences, before);
    }

    #[test]
    fn every_page_has_a_label() {
        for tab in TABS {
            assert!(!tab.label().is_empty());
        }
    }
}
