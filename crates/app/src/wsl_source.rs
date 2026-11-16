//! Finding the installed WSL distributions and the command that opens a terminal in one.
//! Only Windows has WSL; elsewhere there is nothing to find.

use remotething_core::WslDistro;

use crate::i18n::fl;

/// The installed distributions, and which run. Runs `wsl.exe`, which can take a few
/// seconds, so this is not for the window's thread.
pub fn installed_distributions() -> Result<Vec<WslDistro>, String> {
    installed_distributions_impl()
}

/// The program and arguments that open a terminal in `distribution`, the way Windows
/// Terminal does: `wsl.exe -d NAME`. A distribution that is not running starts.
pub fn terminal_command(distribution: &str) -> Result<(String, Vec<String>), String> {
    if !cfg!(windows) {
        return Err(fl!("wsl-windows-only"));
    }
    Ok((
        "wsl.exe".to_string(),
        vec!["-d".to_string(), distribution.to_string()],
    ))
}

#[cfg(not(windows))]
fn installed_distributions_impl() -> Result<Vec<WslDistro>, String> {
    Err(fl!("wsl-windows-only"))
}

#[cfg(windows)]
fn installed_distributions_impl() -> Result<Vec<WslDistro>, String> {
    remotething_core::discover_wsl(&mut windows::Wsl)
}

#[cfg(windows)]
mod windows {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    use remotething_core::{WslRunner, decode_wsl_output};

    use crate::i18n::fl;

    /// Keeps a console window from flashing up for every command.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub(super) struct Wsl;

    impl WslRunner for Wsl {
        fn run(&mut self, arguments: &[&str]) -> Result<Vec<u8>, String> {
            let output = Command::new("wsl.exe")
                .args(arguments)
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|error| format!("{} {error}", fl!("wsl-not-installed")))?;
            if output.status.success() {
                return Ok(output.stdout);
            }
            // What wsl.exe says is the reason.
            let said = if output.stdout.is_empty() {
                output.stderr
            } else {
                output.stdout
            };
            Err(decode_wsl_output(&said).trim().to_string())
        }
    }
}
