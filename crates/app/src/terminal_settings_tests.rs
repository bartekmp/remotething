//! Tests of `terminal_settings.rs`.

use super::*;

const KITTY: &str = "foreground #ebdbb2\nbackground #282828\ncolor1 #cc241d\n";

struct Fixture {
    directory: tempfile::TempDir,
    state: State,
    preferences: Preferences,
}

impl Fixture {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
            state: State::default(),
            preferences: Preferences::default(),
        }
    }

    fn connections(&self) -> PathBuf {
        self.directory.path().join("connections.toml")
    }

    fn send(&mut self, message: Message) -> Outcome {
        let connections = self.connections();
        update(
            &mut self.state,
            &mut self.preferences,
            &connections,
            message,
        )
    }

    fn import(&mut self, file_name: &str, text: &str) {
        let path = self.directory.path().join(file_name);
        std::fs::write(&path, text).unwrap();
        self.send(Message::FilePicked(Some(path)));
    }
}

#[test]
fn an_imported_theme_is_listed_saved_and_shown_in_the_preview() {
    let mut fixture = Fixture::new();

    fixture.import("gruvbox.conf", KITTY);

    assert!(fixture.state.library.get("gruvbox").is_some());
    assert_eq!(fixture.state.previewed.as_deref(), Some("gruvbox"));
    let (reloaded, problem) = State::load(&fixture.connections());
    assert!(problem.is_none());
    assert!(reloaded.library.get("gruvbox").is_some());
    // Looking is not choosing.
    assert_eq!(fixture.preferences.terminal.theme, None);
}

#[test]
fn a_file_that_is_no_theme_says_why_and_changes_nothing() {
    let mut fixture = Fixture::new();

    fixture.import("junk.conf", "hello");

    assert!(fixture.state.library.themes.is_empty());
    assert!(
        fixture
            .state
            .notice
            .as_deref()
            .unwrap()
            .contains("Cannot import")
    );
}

#[test]
fn using_the_previewed_theme_changes_the_preferences_and_the_default_can_be_chosen_again() {
    let mut fixture = Fixture::new();
    fixture.import("gruvbox.conf", KITTY);

    let outcome = fixture.send(Message::ApplyPressed);
    assert!(outcome.preferences_changed);
    assert_eq!(
        fixture.preferences.terminal.theme.as_deref(),
        Some("gruvbox")
    );
    assert!(
        fixture
            .state
            .applied_palette(&fixture.preferences)
            .is_some()
    );

    fixture.send(Message::Previewed(None));
    fixture.send(Message::ApplyPressed);
    assert_eq!(fixture.preferences.terminal.theme, None);
    assert!(
        fixture
            .state
            .applied_palette(&fixture.preferences)
            .is_none()
    );
}

#[test]
fn deleting_the_theme_in_use_goes_back_to_the_default() {
    let mut fixture = Fixture::new();
    fixture.import("gruvbox.conf", KITTY);
    fixture.send(Message::ApplyPressed);

    let outcome = fixture.send(Message::DeletePressed);

    assert!(outcome.preferences_changed);
    assert!(fixture.state.library.themes.is_empty());
    assert_eq!(fixture.preferences.terminal.theme, None);
    assert_eq!(fixture.state.previewed, None);
}

#[test]
fn a_chosen_theme_that_is_gone_from_the_library_falls_back_to_the_default_palette() {
    let mut fixture = Fixture::new();
    fixture.preferences.terminal.theme = Some("vanished".to_string());

    assert!(
        fixture
            .state
            .applied_palette(&fixture.preferences)
            .is_none()
    );
}

#[test]
fn the_text_size_is_a_preference_and_the_import_button_asks_for_a_file() {
    let mut fixture = Fixture::new();

    let outcome = fixture.send(Message::FontSizeChanged(18));
    assert!(outcome.preferences_changed);
    assert_eq!(fixture.preferences.terminal.font_size, 18);
    assert!(fixture.send(Message::ImportPressed).pick_file);
}

#[test]
fn a_font_name_is_applied_on_enter_and_a_blank_one_restores_the_bundled_font() {
    let mut fixture = Fixture::new();

    assert!(
        !fixture
            .send(Message::FontEdited("Fira Code".into()))
            .preferences_changed
    );
    assert_eq!(fixture.preferences.terminal.font_family, None);

    assert!(fixture.send(Message::FontSubmitted).preferences_changed);
    assert_eq!(
        fixture.preferences.terminal.font_family.as_deref(),
        Some("Fira Code")
    );

    fixture.send(Message::FontEdited("  ".into()));
    fixture.send(Message::FontSubmitted);
    assert_eq!(fixture.preferences.terminal.font_family, None);
}

#[test]
fn the_preview_uses_every_colour_of_the_palette_and_lines_up_the_logo() {
    let lines = fastfetch();

    let ansi: std::collections::HashSet<u8> = lines
        .iter()
        .flatten()
        .filter_map(|segment| match segment.paint {
            Paint::Ansi(index) => Some(index),
            _ => None,
        })
        .collect();
    assert_eq!(ansi.len(), 16);
    // Every line starts with a logo cell of the same width.
    let widths: std::collections::HashSet<usize> = lines
        .iter()
        .map(|line| line[0].text.chars().count())
        .collect();
    assert_eq!(widths.len(), 1);
}

#[test]
fn imported_themes_are_listed_before_the_built_in_ones() {
    let mut fixture = Fixture::new();

    fixture.import("zzz.conf", KITTY);

    let names = fixture.state.names();
    assert_eq!(names[0], "zzz");
    assert_eq!(names.len(), crate::terminal_builtin::all().len() + 1);
}

#[test]
fn a_built_in_theme_cannot_be_deleted() {
    let mut fixture = Fixture::new();
    let built_in = crate::terminal_builtin::all()[0].name.clone();
    fixture.send(Message::Previewed(Some(built_in.clone())));

    fixture.send(Message::DeletePressed);

    assert!(fixture.state.names().contains(&built_in));
}
