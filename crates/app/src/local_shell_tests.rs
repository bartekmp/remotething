use super::*;

#[test]
fn the_folder_defaults_to_home_and_understands_the_tilde() {
    let home = Some(PathBuf::from("/home/bartek"));

    assert_eq!(directory(None, home.clone()), home);
    assert_eq!(directory(Some("  "), home.clone()), home);
    assert_eq!(directory(Some("~"), home.clone()), home);
    assert_eq!(
        directory(Some("~/src"), home.clone()),
        Some(PathBuf::from("/home/bartek/src"))
    );
    assert_eq!(
        directory(Some("/srv/app"), home),
        Some(PathBuf::from("/srv/app"))
    );
}

#[cfg(not(windows))]
#[test]
fn the_default_program_is_the_users_shell_or_sh() {
    assert_eq!(
        default_program_from(Some("/usr/bin/zsh".to_string()), None, &[]),
        "/usr/bin/zsh"
    );
    assert_eq!(
        default_program_from(Some(String::new()), None, &[]),
        "/bin/sh"
    );
    assert_eq!(default_program_from(None, None, &[]), "/bin/sh");
}

#[test]
fn arguments_keep_what_is_quoted_together() {
    let words = split_words(r#"-l --rcfile "my rc""#).unwrap();

    assert_eq!(words, ["-l", "--rcfile", "my rc"]);
    assert_eq!(split_words(&join_words(&words)).unwrap(), words);
    assert!(split_words("\"open").is_err());
}

#[test]
fn variables_are_names_and_values() {
    let variables = parse_environment(r#"LANG=pl_PL.UTF-8 GREETING="hello there" EMPTY="#).unwrap();

    assert_eq!(variables["LANG"], "pl_PL.UTF-8");
    assert_eq!(variables["GREETING"], "hello there");
    assert_eq!(variables["EMPTY"], "");
    assert_eq!(
        parse_environment(&format_environment(&variables)).unwrap(),
        variables
    );
    assert_eq!(parse_environment("NOVALUE").unwrap_err(), "NOVALUE");
    assert_eq!(parse_environment("=x").unwrap_err(), "=x");
}

#[test]
fn a_connection_without_a_shell_starts_the_default_one() {
    let connection = ResolvedConnection {
        terminal_theme: None,
        id: String::new(),
        name: String::new(),
        protocol: remotething_core::Protocol::Local,
        host: String::new(),
        port: 0,
        credential: None,
        jump: None,
        forwards: Vec::new(),
        local: remotething_core::LocalSettings::default(),
        desktop_quality: remotething_core::Quality::default(),
    };

    let launch = launch(&connection);

    assert_eq!(launch.program, default_program());
    assert_eq!(launch.directory, home());
}
