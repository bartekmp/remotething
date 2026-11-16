//! Finding the running WSL distributions. Only Windows has WSL; elsewhere there is nothing to find.

use remotething_core::WslDistro;

use crate::i18n::fl;

/// The running distributions, with their addresses and users. Runs `wsl.exe`, which can take
/// a few seconds, so this is not for the window's thread.
pub fn running_distributions() -> Result<Vec<WslDistro>, String> {
    running_distributions_impl()
}

#[cfg(not(windows))]
fn running_distributions_impl() -> Result<Vec<WslDistro>, String> {
    Err(fl!("wsl-windows-only"))
}

#[cfg(windows)]
fn running_distributions_impl() -> Result<Vec<WslDistro>, String> {
    remotething_core::discover_wsl(&mut windows::Wsl)
}

#[cfg(windows)]
mod windows {
    use std::net::{SocketAddr, TcpStream};
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::time::Duration;

    use remotething_core::{WslRunner, decode_wsl_output};

    use crate::i18n::fl;

    /// Keeps a console window from flashing up for every command.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    /// How long a port gets to answer.
    const SSH_WAIT: Duration = Duration::from_millis(700);

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
            // What wsl.exe says, such as that no distribution is running, is the reason.
            let said = if output.stdout.is_empty() {
                output.stderr
            } else {
                output.stdout
            };
            Err(decode_wsl_output(&said).trim().to_string())
        }

        fn ssh_answers(&mut self, address: &str) -> bool {
            let Ok(address) = format!("{address}:22").parse::<SocketAddr>() else {
                return false;
            };
            TcpStream::connect_timeout(&address, SSH_WAIT).is_ok()
        }
    }
}
