//! Where PuTTY keeps its sessions, and how to read them. Only reads; PuTTY's data is never changed.

use std::path::Path;

use remotething_core::{PuttySession, decode_reg_bytes, parse_reg};

/// The sessions PuTTY has on this computer.
///
/// On Linux that is a file per session in `~/.putty/sessions`. On Windows
/// they are in the registry, which is read through `reg export` so the same
/// reader serves both a live registry and a `.reg` file.
pub fn default_sessions() -> Result<Vec<PuttySession>, String> {
    default_sessions_impl()
}

#[cfg(not(windows))]
fn default_sessions_impl() -> Result<Vec<PuttySession>, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    sessions_in_directory(
        &std::path::PathBuf::from(home)
            .join(".putty")
            .join("sessions"),
    )
}

#[cfg(windows)]
fn default_sessions_impl() -> Result<Vec<PuttySession>, String> {
    let file = std::env::temp_dir().join(format!("remotething-putty-{}.reg", std::process::id()));
    let status = std::process::Command::new("reg")
        .args(["export", r"HKCU\Software\SimonTatham\PuTTY\Sessions"])
        .arg(&file)
        .arg("/y")
        .output()
        .map_err(|error| error.to_string())?;
    if !status.status.success() {
        return Err("PuTTY has no saved sessions in the registry".to_string());
    }
    let sessions = sessions_in_reg_file(&file);
    let _ = std::fs::remove_file(&file);
    sessions
}

/// Every session file in a folder, by name. Windows keeps sessions in the registry instead.
#[cfg(any(not(windows), test))]
pub fn sessions_in_directory(directory: &Path) -> Result<Vec<PuttySession>, String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        // A file that cannot be read is left out, like a session that is not SSH.
        if let Ok(text) = std::fs::read_to_string(entry.path()) {
            sessions.push(remotething_core::parse_session_file(&name, &text));
        }
    }
    sessions.sort_by(|first, second| first.name.cmp(&second.name));
    Ok(sessions)
}

/// The sessions in a `.reg` file exported from PuTTY's registry key.
pub fn sessions_in_reg_file(file: &Path) -> Result<Vec<PuttySession>, String> {
    let bytes =
        std::fs::read(file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
    let sessions = parse_reg(&decode_reg_bytes(&bytes));
    if sessions.is_empty() {
        return Err(format!("{} holds no PuTTY sessions", file.display()));
    }
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_of_session_files_is_read_by_name() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("Web%2001"),
            "HostName=web.example.net\n",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("Db"),
            "HostName=db.example.net\nPortNumber=2200\n",
        )
        .unwrap();

        let sessions = sessions_in_directory(directory.path()).unwrap();

        let names: Vec<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Db", "Web 01"]);
        assert_eq!(sessions[0].port, Some(2200));
    }

    #[test]
    fn a_missing_folder_says_which_one() {
        let error = sessions_in_directory(Path::new("/nonexistent/putty")).unwrap_err();

        assert!(error.contains("/nonexistent/putty"));
    }

    #[test]
    fn a_reg_file_without_sessions_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("other.reg");
        std::fs::write(&file, "Windows Registry Editor Version 5.00\n").unwrap();

        assert!(sessions_in_reg_file(&file).is_err());
    }

    #[test]
    fn a_reg_file_gives_its_sessions() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("putty.reg");
        let text = "[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Web]\n\"HostName\"=\"web.example.net\"\n";
        std::fs::write(&file, text).unwrap();

        let sessions = sessions_in_reg_file(&file).unwrap();

        assert_eq!(sessions[0].host.as_deref(), Some("web.example.net"));
    }
}
