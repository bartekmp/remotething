//! Shells on this machine: which are installed, and how one is started for a connection.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use remotething_core::ResolvedConnection;

use crate::i18n::fl;
use remotething_remote::local::Launch;

/// Programs worth offering, when they are installed: shells, then terminal multiplexers.
#[cfg(not(windows))]
const KNOWN: [&str; 11] = [
    "bash", "zsh", "fish", "sh", "dash", "nu", "pwsh", "tmux", "zellij", "screen", "herdr",
];
#[cfg(windows)]
const KNOWN: [&str; 6] = ["pwsh", "powershell", "cmd", "bash", "nu", "wsl"];

/// The shells and programs found on the search path, by name.
pub fn installed() -> &'static [String] {
    static FOUND: OnceLock<Vec<String>> = OnceLock::new();
    FOUND.get_or_init(|| {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let folders: Vec<PathBuf> = std::env::split_paths(&path).collect();
        KNOWN
            .iter()
            .filter(|name| folders.iter().any(|folder| is_program(folder, name)))
            .map(|name| name.to_string())
            .collect()
    })
}

fn is_program(folder: &Path, name: &str) -> bool {
    let file = folder.join(name);
    file.is_file() || folder.join(format!("{name}.exe")).is_file()
}

/// The program of a connection that names none: the user's own shell.
pub fn default_program() -> String {
    default_program_from(
        std::env::var("SHELL").ok(),
        std::env::var("COMSPEC").ok(),
        installed(),
    )
}

fn default_program_from(
    shell: Option<String>,
    comspec: Option<String>,
    found: &[String],
) -> String {
    if cfg!(windows) {
        let modern = ["pwsh", "powershell"]
            .into_iter()
            .find(|name| found.iter().any(|known| known == name));
        return modern
            .map(str::to_string)
            .or(comspec)
            .unwrap_or_else(|| "cmd.exe".to_string());
    }
    shell
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| "/bin/sh".to_string())
}

/// The name of a tab with a local shell: the shell's own name, or "Terminal" for the user's.
pub fn title(host: &str) -> String {
    let host = host.trim();
    if host.is_empty() {
        return fl!("local-terminal-title");
    }
    Path::new(host).file_stem().map_or_else(
        || host.to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The home folder of the user, where shells start unless told otherwise.
pub fn home() -> Option<PathBuf> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|home| !home.as_os_str().is_empty())
}

/// What to start for a local connection.
pub fn launch(connection: &ResolvedConnection) -> Launch {
    let host = connection.host.trim();
    let settings = &connection.local;
    Launch {
        program: if host.is_empty() {
            default_program()
        } else {
            host.to_string()
        },
        arguments: settings.arguments.clone(),
        directory: directory(settings.directory.as_deref(), home()),
        environment: settings
            .environment
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
    }
}

/// The folder to start in: the one typed, with `~` for the home folder, or the home folder.
fn directory(typed: Option<&str>, home: Option<PathBuf>) -> Option<PathBuf> {
    let typed = typed.map(str::trim).filter(|typed| !typed.is_empty());
    let Some(typed) = typed else {
        return home;
    };
    if typed == "~" {
        return home;
    }
    match typed
        .strip_prefix("~/")
        .or_else(|| typed.strip_prefix("~\\"))
    {
        Some(rest) => home.map(|home| home.join(rest)),
        None => Some(PathBuf::from(typed)),
    }
}

/// Words as typed, with quotes for the ones that hold spaces: `-l --init-file "my rc"`.
pub fn split_words(text: &str) -> Result<Vec<String>, String> {
    shell_words::split(text).map_err(|error| error.to_string())
}

/// Words joined back the way `split_words` reads them.
pub fn join_words(words: &[String]) -> String {
    shell_words::join(words)
}

/// Variables as typed: `LANG=pl_PL.UTF-8 EDITOR=vim`.
pub fn parse_environment(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut variables = BTreeMap::new();
    for word in split_words(text)? {
        match word.split_once('=') {
            Some((name, value)) if !name.is_empty() => {
                variables.insert(name.to_string(), value.to_string());
            }
            _ => return Err(word),
        }
    }
    Ok(variables)
}

/// Variables as `parse_environment` reads them.
pub fn format_environment(variables: &BTreeMap<String, String>) -> String {
    let words: Vec<String> = variables
        .iter()
        .map(|(name, value)| format!("{name}={}", shell_words::quote(value)))
        .collect();
    words.join(" ")
}

#[cfg(test)]
#[path = "local_shell_tests.rs"]
mod tests;
