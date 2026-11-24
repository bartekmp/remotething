//! The WSL distributions installed on a Windows machine and what they become in
//! the tree. Nothing here starts a process: the caller runs `wsl.exe` (see
//! [`discover`]) and hands the output in, so everything can be checked on any system.
//!
//! A distribution is reached the way Windows Terminal does it, by running
//! `wsl.exe -d NAME` in a local terminal. That needs no address and no SSH server.

use crate::Result;
use crate::model::{Connection, Folder, Protocol, Settings, Store};

/// The name of the folder the import creates, and adds to when it is there.
const FOLDER_NAME: &str = "WSL";
/// Distributions Docker Desktop keeps for itself; nobody opens a shell in them.
const DOCKER_PREFIX: &str = "docker-desktop";

/// An installed distribution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WslDistro {
    /// The distribution's name, such as `Ubuntu-24.04`.
    pub name: String,
    /// Whether it is running now. A stopped one starts when it is connected to.
    pub running: bool,
}

/// What `wsl.exe` printed, as text. It writes UTF-16 (little endian, usually with a
/// byte order mark); anything that does not look like that is read as UTF-8.
pub fn decode_wsl_output(bytes: &[u8]) -> String {
    let bom = bytes.starts_with(&[0xFF, 0xFE]);
    let zeros_at_odd_places = bytes.len() >= 4
        && bytes
            .iter()
            .skip(1)
            .step_by(2)
            .take(8)
            .filter(|byte| **byte == 0)
            .count()
            >= 3;
    if bom || zeros_at_odd_places {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        let text = String::from_utf16_lossy(&units);
        return text.trim_start_matches('\u{feff}').to_string();
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// The names in the output of `wsl.exe --list --quiet` (or with `--running`): one per
/// line, no headers, so the words of the user's language do not matter.
pub fn parse_names(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|line| line.trim().trim_matches('\u{0}'))
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// The ways to ask `wsl.exe`, for [`discover`].
pub trait WslRunner {
    /// Runs `wsl.exe` with `arguments` and returns what it printed.
    fn run(&mut self, arguments: &[&str]) -> std::result::Result<Vec<u8>, String>;
}

/// Finds the installed distributions and which of them run.
pub fn discover(runner: &mut dyn WslRunner) -> std::result::Result<Vec<WslDistro>, String> {
    let installed = parse_names(&decode_wsl_output(&runner.run(&["--list", "--quiet"])?));
    // With nothing running `wsl.exe` can say so as an error; that is just an empty list.
    let running = runner
        .run(&["--list", "--running", "--quiet"])
        .map(|bytes| parse_names(&decode_wsl_output(&bytes)))
        .unwrap_or_default();
    Ok(installed
        .into_iter()
        .map(|name| WslDistro {
            running: running.contains(&name),
            name,
        })
        .collect())
}

/// A distribution the import would turn into a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWsl {
    /// The connection's name: the distribution's.
    pub name: String,
    /// Whether it is running now.
    pub running: bool,
}

/// A distribution that will not be imported, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WslSkipped {
    /// The distribution's name.
    pub name: String,
    /// The reason, in words for the user.
    pub reason: String,
}

/// What an import would do, before anything is written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WslPlan {
    /// Distributions that become connections.
    pub connections: Vec<PlannedWsl>,
    /// Distributions that cannot be imported.
    pub skipped: Vec<WslSkipped>,
}

/// Works out what importing `distros` would do.
pub fn plan_wsl(distros: &[WslDistro]) -> WslPlan {
    let mut plan = WslPlan::default();
    for distro in distros {
        if distro.name.starts_with(DOCKER_PREFIX) {
            plan.skipped.push(WslSkipped {
                name: distro.name.clone(),
                reason: "Docker Desktop keeps it for itself".to_string(),
            });
            continue;
        }
        plan.connections.push(PlannedWsl {
            name: distro.name.clone(),
            running: distro.running,
        });
    }
    plan
}

/// What an import changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WslImported {
    /// Connections that were new.
    pub added: usize,
    /// Distributions that were chosen but already had a connection.
    pub already_there: usize,
}

impl Store {
    /// Adds the distributions of `plan` named in `chosen` to the folder `WSL`, made when it
    /// is missing. A distribution that already has a connection of its name in that folder is
    /// left as it is.
    pub fn import_wsl(&mut self, plan: &WslPlan, chosen: &[String]) -> Result<WslImported> {
        let wanted: Vec<&PlannedWsl> = plan
            .connections
            .iter()
            .filter(|connection| chosen.contains(&connection.name))
            .collect();
        if wanted.is_empty() {
            return Ok(WslImported::default());
        }
        let mut merged = self.clone();
        let folder_id = match merged
            .folders
            .iter()
            .find(|folder| folder.name == FOLDER_NAME && folder.parent.is_none())
        {
            Some(folder) => folder.id.clone(),
            None => {
                let id = merged.new_folder_id(FOLDER_NAME);
                merged.folders.push(Folder {
                    id: id.clone(),
                    name: FOLDER_NAME.to_string(),
                    parent: None,
                    icon: None,
                    settings: Settings::default(),
                });
                id
            }
        };

        let mut imported = WslImported::default();
        for distro in wanted {
            let exists = merged.connections.iter().any(|connection| {
                connection.parent.as_deref() == Some(folder_id.as_str())
                    && connection.name == distro.name
            });
            if exists {
                imported.already_there += 1;
                continue;
            }
            merged.connections.push(Connection {
                id: merged.new_connection_id(&distro.name),
                name: distro.name.clone(),
                parent: Some(folder_id.clone()),
                protocol: Protocol::Wsl,
                host: distro.name.clone(),
                icon: None,
                settings: Settings::default(),
            });
            imported.added += 1;
        }

        merged.validate()?;
        *self = merged;
        Ok(imported)
    }
}

#[cfg(test)]
#[path = "wsl_tests.rs"]
mod tests;
