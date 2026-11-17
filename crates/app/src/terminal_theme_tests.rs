//! Tests of `terminal_theme.rs`.

use super::*;

const ALACRITTY: &str = r##"
[colors.primary]
background = "#282828"
foreground = "0xebdbb2"

[colors.selection]
background = "#504945"

[colors.normal]
black = "#282828"
red = "#cc241d"
green = "#98971a"
yellow = "#d79921"
blue = "#458588"
magenta = "#b16286"
cyan = "#689d6a"
white = "#a89984"

[colors.bright]
black = "#928374"
red = "#fb4934"
green = "#b8bb26"
yellow = "#fabd2f"
blue = "#83a598"
magenta = "#d3869b"
cyan = "#8ec07c"
white = "#ebdbb2"
"##;

const KITTY: &str = "# a comment\nforeground #ebdbb2\nbackground  #282828\n\
selection_background #504945\ncolor0 #282828\ncolor1 #cc241d\ncolor8 #928374\ncolor15 #ebdbb2\n\
color99 #ffffff\ncursor #ffffff\n";

const GHOSTTY: &str = "# a comment\nbackground = 282828\nforeground = #ebdbb2\n\
selection-background = #504945\npalette = 0=#282828\npalette = 1=#cc241d\npalette = 15=#ebdbb2\n\
cursor-color = #ffffff\n";

#[test]
fn an_alacritty_file_gives_every_colour() {
    let theme = parse("gruvbox.toml", ALACRITTY).unwrap();

    assert_eq!(theme.name, "gruvbox");
    assert_eq!(String::from(theme.foreground), "#ebdbb2");
    assert_eq!(String::from(theme.background), "#282828");
    assert_eq!(
        theme.selection.map(String::from).as_deref(),
        Some("#504945")
    );
    assert_eq!(theme.ansi.len(), 16);
    assert_eq!(String::from(theme.ansi[1]), "#cc241d");
    assert_eq!(String::from(theme.ansi[9]), "#fb4934");
}

#[test]
fn a_kitty_file_gives_the_colours_it_names_and_the_rest_come_from_the_default() {
    let theme = parse("gruvbox.conf", KITTY).unwrap();

    assert_eq!(String::from(theme.foreground), "#ebdbb2");
    assert_eq!(String::from(theme.ansi[8]), "#928374");
    assert_eq!(theme.ansi.len(), 16);
    // Colour 2 was not given: it is the default dark green.
    assert_eq!(String::from(theme.ansi[2]), "#8fd694");
}

#[test]
fn a_ghostty_file_is_told_by_its_palette_lines() {
    let theme = parse("gruvbox", GHOSTTY).unwrap();

    assert_eq!(theme.name, "gruvbox");
    assert_eq!(String::from(theme.background), "#282828");
    assert_eq!(String::from(theme.ansi[15]), "#ebdbb2");
    assert_eq!(
        theme.selection.map(String::from).as_deref(),
        Some("#504945")
    );
}

#[test]
fn files_that_are_not_themes_say_what_is_missing() {
    assert!(parse("x.toml", "[colors.primary]\nbackground = \"#000000\"\n").is_err());
    assert!(parse("x.conf", "background #000000\nforeground #ffffff\n").is_err());
    assert!(parse("x.conf", "hello world").is_err());
    assert!(parse("x.toml", "not toml [").is_err());
}

#[test]
fn colours_are_read_with_or_without_a_prefix_and_bad_ones_are_refused() {
    assert_eq!(Rgb::parse("#AbCdEf"), Rgb::parse("0xabcdef"));
    assert_eq!(Rgb::parse("abcdef"), Rgb::parse("\"#abcdef\""));
    assert!(Rgb::parse("#abc").is_none());
    assert!(Rgb::parse("#gggggg").is_none());
}

#[test]
fn the_palette_has_the_theme_colours_and_light_backgrounds_get_light_defaults() {
    let dark = parse("d.conf", KITTY).unwrap().palette();
    assert_eq!(dark.background, Color::from_rgb8(0x28, 0x28, 0x28));
    assert_eq!(dark.ansi[1], Color::from_rgb8(0xcc, 0x24, 0x1d));
    assert_eq!(dark.ansi[2], Palette::default_for(true).ansi[2]);

    let light = "foreground #111111\nbackground #ffffff\ncolor1 #ff0000\n";
    let light = parse("l.conf", light).unwrap().palette();
    assert_eq!(light.ansi[2], Palette::default_for(false).ansi[2]);
    assert_eq!(light.selection, Palette::default_for(false).selection);
}

fn named(name: &str) -> Theme {
    Theme {
        name: name.to_string(),
        ..parse("x.conf", KITTY).unwrap()
    }
}

#[test]
fn adding_a_theme_keeps_the_list_sorted_and_replaces_the_same_name() {
    let mut library = Library::default();
    library.add(named("nord"));
    library.add(named("Gruvbox"));
    library.add(named("nord"));

    let names: Vec<&str> = library.themes.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Gruvbox", "nord"]);
    library.remove("nord");
    assert!(library.get("nord").is_none() && library.get("Gruvbox").is_some());
}

#[test]
fn the_library_is_saved_and_read_back_and_a_missing_file_is_empty() {
    let directory = tempfile::tempdir().unwrap();
    let path = path_beside(&directory.path().join("connections.toml"));
    assert_eq!(load(&path).unwrap(), Library::default());

    let mut library = Library::default();
    library.add(parse("gruvbox.toml", ALACRITTY).unwrap());
    save(&library, &path).unwrap();

    assert_eq!(load(&path).unwrap(), library);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("#ebdbb2"), "{text}");
}

#[test]
fn a_broken_library_is_an_error_not_an_empty_list() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("terminal-themes.toml");
    std::fs::write(&path, "[[theme]]\nname = 3\n").unwrap();

    assert!(load(&path).is_err());
}
