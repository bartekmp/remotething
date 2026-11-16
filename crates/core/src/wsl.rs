//! Running WSL distributions and what they become in the tree. Nothing here
//! starts a process: the caller runs `wsl.exe` (see [`discover`]) and hands
//! the output in, so everything can be checked on any system.

use crate::Result;
use crate::model::{Connection, Credential, CredentialKind, Folder, Protocol, Settings, Store};

/// The name of the folder the import creates, and adds to when it is there.
const FOLDER_NAME: &str = "WSL";
/// Distributions Docker Desktop keeps for itself; they run no SSH server.
const DOCKER_PREFIX: &str = "docker-desktop";

/// A running distribution and what could be found out about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WslDistro {
    /// The distribution's name, such as `Ubuntu-24.04`.
    pub name: String,
    /// Its IPv4 address as seen from Windows.
    pub address: Option<String>,
    /// The user a shell in it starts as.
    pub user: Option<String>,
    /// Whether something answered on port 22, when that was tried.
    pub ssh_answers: Option<bool>,
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
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let text = String::from_utf16_lossy(&units);
        return text.trim_start_matches('\u{feff}').to_string();
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// The names in the output of `wsl.exe --list --running --quiet`: one per line,
/// no headers, so the words of the user's language do not matter.
pub fn parse_running(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|line| line.trim().trim_matches('\u{0}'))
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// The first IPv4 address in the output of `hostname -I`.
pub fn first_ipv4(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|word| word.parse::<std::net::Ipv4Addr>().is_ok())
        .map(str::to_string)
}

/// The user name in the output of `whoami`.
pub fn parse_user(output: &str) -> Option<String> {
    let user = output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    // A name with spaces or odd characters is not a user name, but a message.
    let plain = user
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '$'));
    plain.then(|| user.to_string())
}

/// The ways a command can be run in a distribution, for [`discover`].
pub trait WslRunner {
    /// Runs `wsl.exe` with `arguments` and returns what it printed.
    fn run(&mut self, arguments: &[&str]) -> std::result::Result<Vec<u8>, String>;
    /// Whether something answers at `address` on the SSH port.
    fn ssh_answers(&mut self, address: &str) -> bool;
}

/// Finds the running distributions and, for each, its address and user.
pub fn discover(runner: &mut dyn WslRunner) -> std::result::Result<Vec<WslDistro>, String> {
    let listed = runner.run(&["--list", "--running", "--quiet"])?;
    let mut distros = Vec::new();
    for name in parse_running(&decode_wsl_output(&listed)) {
        let mut distro = WslDistro {
            name: name.clone(),
            ..WslDistro::default()
        };
        // What runs inside prints plain UTF-8, unlike wsl.exe itself.
        let inside = |runner: &mut dyn WslRunner, command: &[&str]| {
            let mut arguments = vec!["-d", name.as_str(), "--"];
            arguments.extend_from_slice(command);
            runner
                .run(&arguments)
                .ok()
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        };
        distro.address = inside(runner, &["hostname", "-I"]).and_then(|text| first_ipv4(&text));
        distro.user = inside(runner, &["whoami"]).and_then(|text| parse_user(&text));
        if let Some(address) = &distro.address {
            distro.ssh_answers = Some(runner.ssh_answers(address));
        }
        distros.push(distro);
    }
    Ok(distros)
}

/// A distribution the import would turn into a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWsl {
    /// The connection's name: the distribution's.
    pub name: String,
    /// Its address.
    pub address: String,
    /// The user to sign in as, when known.
    pub user: Option<String>,
    /// What the user should know, in words.
    pub notes: Vec<String>,
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
                reason: "Docker Desktop keeps it for itself, and it has no SSH server".to_string(),
            });
            continue;
        }
        let Some(address) = distro.address.clone() else {
            plan.skipped.push(WslSkipped {
                name: distro.name.clone(),
                reason: "its address could not be found".to_string(),
            });
            continue;
        };
        let mut notes = Vec::new();
        if distro.ssh_answers == Some(false) {
            notes.push(
                "nothing answers on port 22 yet; install and start an SSH server in it, \
                 for example: sudo apt install openssh-server && sudo service ssh start"
                    .to_string(),
            );
        }
        plan.connections.push(PlannedWsl {
            name: distro.name.clone(),
            address,
            user: distro.user.clone().filter(|user| user != "root"),
            notes,
        });
    }
    plan
}

/// What an import changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WslImported {
    /// Connections that were new.
    pub added: usize,
    /// Connections that existed and got a new address.
    pub updated: usize,
}

impl Store {
    /// Adds the distributions of `plan` named in `chosen` to the folder `WSL`, made when it
    /// is missing. A connection of the same name in that folder only gets the new address:
    /// a distribution has another one each time it starts.
    ///
    /// A user shares one credential that asks for the password, as PuTTY's import does.
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
            let existing = merged.connections.iter_mut().find(|connection| {
                connection.parent.as_deref() == Some(folder_id.as_str())
                    && connection.name == distro.name
            });
            if let Some(connection) = existing {
                if connection.host != distro.address {
                    connection.host = distro.address.clone();
                    imported.updated += 1;
                }
                continue;
            }
            let credential = distro
                .user
                .as_ref()
                .map(|user| credential_for(&mut merged, user));
            merged.connections.push(Connection {
                id: merged.new_connection_id(&distro.name),
                name: distro.name.clone(),
                parent: Some(folder_id.clone()),
                protocol: Protocol::Ssh,
                host: distro.address.clone(),
                icon: Some("terminal".to_string()),
                settings: Settings {
                    credential,
                    ..Settings::default()
                },
            });
            imported.added += 1;
        }

        merged.validate()?;
        *self = merged;
        Ok(imported)
    }
}

/// The credential of a WSL user: the one made before, or a new one that asks for the password.
fn credential_for(store: &mut Store, user: &str) -> String {
    let name = format!("{user} (WSL)");
    if let Some(existing) = store
        .credentials
        .iter()
        .find(|credential| credential.name == name && credential.username.as_deref() == Some(user))
    {
        return existing.id.clone();
    }
    let id = store.new_credential_id(&name);
    store.credentials.push(Credential {
        id: id.clone(),
        name,
        username: Some(user.to_string()),
        domain: None,
        kind: CredentialKind::Ask,
        key_file: None,
    });
    id
}

#[cfg(test)]
#[path = "wsl_tests.rs"]
mod tests;
