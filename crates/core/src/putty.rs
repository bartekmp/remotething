//! Reading PuTTY sessions, from a folder of session files or a `.reg` export,
//! and planning what they become in the tree. PuTTY's own data is only read.

use std::collections::{BTreeMap, HashMap};

use crate::forward::Forward;
use crate::model::{Connection, Credential, CredentialKind, Folder, Protocol, Settings, Store};
use crate::{Error, Result};

/// The name of PuTTY's template session; its values apply to the others.
const DEFAULT_SESSION: &str = "Default Settings";
/// The registry path in a `.reg` export that holds the sessions.
const REG_SESSIONS: &str = r"\Software\SimonTatham\PuTTY\Sessions\";
/// The name of the folder the import creates.
const FOLDER_NAME: &str = "PuTTY";

/// One saved session, as far as it matters here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PuttySession {
    /// The session's name, without PuTTY's percent-encoding.
    pub name: String,
    /// `HostName`.
    pub host: Option<String>,
    /// `PortNumber`.
    pub port: Option<u16>,
    /// `UserName`.
    pub username: Option<String>,
    /// `Protocol`: `ssh`, `telnet`, `serial`, `raw`, `rlogin`; `ssh` when absent.
    pub protocol: Option<String>,
    /// `PublicKeyFile`: the private key file, which for PuTTY is a `.ppk`.
    pub key_file: Option<String>,
    /// `ProxyMethod`: 0 or absent means no proxy.
    pub proxy_method: Option<u32>,
    /// `PortForwardings`, such as `L8080=localhost:80,R9000=host:22`.
    pub forwardings: Option<String>,
}

/// A connection the import would create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedConnection {
    /// The session's name.
    pub name: String,
    /// The host to connect to.
    pub host: String,
    /// The port, when the session sets one.
    pub port: Option<u16>,
    /// The user name, when the session sets one.
    pub username: Option<String>,
    /// The private key file, when the session names one.
    pub key_file: Option<String>,
    /// The local port forwards of the session.
    pub forwards: Vec<Forward>,
    /// What could not be carried over, in words for the user.
    pub notes: Vec<String>,
}

/// A session that will not be imported, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The session's name.
    pub name: String,
    /// The reason, in words for the user.
    pub reason: String,
}

/// What an import would do, before anything is written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportPlan {
    /// Sessions that become connections.
    pub connections: Vec<PlannedConnection>,
    /// Sessions that cannot be imported.
    pub skipped: Vec<Skipped>,
    /// The port from "Default Settings", which becomes the folder's default.
    pub default_port: Option<u16>,
    /// The user name from "Default Settings", which becomes the folder's default.
    pub default_username: Option<String>,
}

/// Undoes PuTTY's `%XX` encoding of session names (`My%20Server` is `My Server`).
pub fn decode_name(encoded: &str) -> String {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                index += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Reads one session file of the Linux layout: `key=value` lines.
pub fn parse_session_file(encoded_name: &str, text: &str) -> PuttySession {
    let values: HashMap<&str, &str> = text
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect();
    session_from(decode_name(encoded_name), |key| {
        values.get(key).map(|value| value.to_string())
    })
}

/// Reads a `.reg` export of PuTTY's sessions, one session per registry key.
///
/// The text must already be decoded; see [`decode_reg_bytes`].
pub fn parse_reg(text: &str) -> Vec<PuttySession> {
    let mut sessions = Vec::new();
    let mut current: Option<(String, HashMap<String, String>)> = None;

    let mut finish = |current: Option<(String, HashMap<String, String>)>| {
        if let Some((name, values)) = current {
            sessions.push(session_from(name, |key| values.get(key).cloned()));
        }
    };
    for line in text.lines().map(str::trim) {
        if let Some(path) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            finish(current.take());
            current = path
                .split_once(REG_SESSIONS)
                .map(|(_, name)| (decode_name(name), HashMap::new()));
        } else if let (Some((_, values)), Some((key, value))) =
            (current.as_mut(), line.split_once('='))
        {
            values.insert(unquote(key), reg_value(value));
        }
    }
    finish(current.take());
    sessions
}

/// The text of a `.reg` file from its bytes. Regedit writes UTF-16 with a
/// byte order mark; other tools write UTF-8.
pub fn decode_reg_bytes(bytes: &[u8]) -> String {
    match bytes {
        [0xFF, 0xFE, rest @ ..] => {
            let units: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .collect();
            String::from_utf16_lossy(&units)
        }
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// A registry value as plain text: `"text"` loses its quotes and escapes,
/// `dword:0000002a` becomes its decimal number.
fn reg_value(value: &str) -> String {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix("dword:") {
        return u32::from_str_radix(hex, 16)
            .map(|number| number.to_string())
            .unwrap_or_default();
    }
    unquote(value)
}

fn unquote(text: &str) -> String {
    let inner = text
        .trim()
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(text.trim());
    inner
        .replace("\\\\", "\u{0}")
        .replace("\\\"", "\"")
        .replace('\u{0}', "\\")
}

fn session_from(name: String, value: impl Fn(&str) -> Option<String>) -> PuttySession {
    let text = |key: &str| value(key).filter(|text| !text.is_empty());
    PuttySession {
        host: text("HostName"),
        port: text("PortNumber").and_then(|port| port.parse().ok()),
        username: text("UserName"),
        protocol: text("Protocol"),
        key_file: text("PublicKeyFile"),
        proxy_method: text("ProxyMethod").and_then(|method| method.parse().ok()),
        forwardings: text("PortForwardings"),
        name,
    }
}

/// Works out what importing `sessions` would do.
pub fn plan(sessions: &[PuttySession]) -> ImportPlan {
    let mut plan = ImportPlan::default();
    for session in sessions {
        if session.name == DEFAULT_SESSION {
            plan.default_port = session.port.filter(|port| *port != 0);
            plan.default_username = session.username.clone();
            continue;
        }
        match planned(session) {
            Ok(connection) => plan.connections.push(connection),
            Err(reason) => plan.skipped.push(Skipped {
                name: session.name.clone(),
                reason,
            }),
        }
    }
    plan
}

fn planned(session: &PuttySession) -> std::result::Result<PlannedConnection, String> {
    let protocol = session.protocol.as_deref().unwrap_or("ssh");
    if !protocol.eq_ignore_ascii_case("ssh") {
        return Err(format!(
            "the {protocol} protocol is not supported, only SSH"
        ));
    }
    let Some(host) = session.host.clone() else {
        return Err("it has no host name".to_string());
    };

    let mut notes = Vec::new();
    if session.proxy_method.is_some_and(|method| method != 0) {
        notes.push("its proxy settings were not imported".to_string());
    }
    let (forwards, left_out) = session
        .forwardings
        .as_deref()
        .map_or((Vec::new(), 0), parse_forwardings);
    if left_out > 0 {
        notes.push(format!(
            "{left_out} port forward(s) could not be read and were not imported"
        ));
    }
    Ok(PlannedConnection {
        name: session.name.clone(),
        host,
        port: session.port.filter(|port| *port != 0),
        username: session.username.clone(),
        key_file: session.key_file.clone(),
        forwards,
        notes,
    })
}

/// Reads PuTTY's `L8080=host:80,R9000=host:9000,D1080`. Returns the forwards
/// and how many entries could not be read.
fn parse_forwardings(text: &str) -> (Vec<Forward>, usize) {
    let mut forwards = Vec::new();
    let mut left_out = 0;
    for entry in text.split(',').filter(|entry| !entry.trim().is_empty()) {
        match forward_of(entry.trim()) {
            Some(forward) => forwards.push(forward),
            None => left_out += 1,
        }
    }
    (forwards, left_out)
}

/// `L8080=host:80`, `R9000=host:80` or `D1080`, optionally with a bind address before the port.
fn forward_of(entry: &str) -> Option<Forward> {
    // The bind address, if any, comes before the last colon.
    let port_of = |source: &str| source.rsplit(':').next().map(str::to_string);
    if let Some(source) = entry.strip_prefix('D') {
        return Forward::parse(&format!("D:{}", port_of(source)?)).ok();
    }
    let (prefix, rest) = if let Some(rest) = entry.strip_prefix('L') {
        ("", rest)
    } else {
        ("R:", entry.strip_prefix('R')?)
    };
    let (source, target) = rest.split_once('=')?;
    Forward::parse(&format!("{prefix}{}:{target}", port_of(source)?)).ok()
}

impl Store {
    /// Adds the connections of `plan` named in `chosen` (by session name) in a
    /// new folder. Nothing that exists is changed.
    ///
    /// Sessions with the same user name and key file share one credential,
    /// created without a secret: PuTTY does not store passwords.
    pub fn import_putty(&mut self, plan: &ImportPlan, chosen: &[String]) -> Result<usize> {
        let mut merged = self.clone();
        let chosen: Vec<&PlannedConnection> = plan
            .connections
            .iter()
            .filter(|connection| chosen.contains(&connection.name))
            .collect();
        if chosen.is_empty() {
            return Ok(0);
        }

        let folder_id = merged.new_folder_id(FOLDER_NAME);
        let default_credential = plan.default_username.as_ref().map(|username| {
            let credential = credential_for(&mut merged, Some(username), None);
            credential.unwrap_or_default()
        });
        merged.folders.push(Folder {
            id: folder_id.clone(),
            name: FOLDER_NAME.to_string(),
            parent: None,
            icon: None,
            settings: Settings {
                port: plan.default_port,
                credential: default_credential.filter(|id| !id.is_empty()),
                ..Settings::default()
            },
        });

        let mut credentials: BTreeMap<(Option<String>, Option<String>), String> = BTreeMap::new();
        for connection in chosen {
            let key = (connection.username.clone(), connection.key_file.clone());
            let credential = match credentials.get(&key) {
                Some(id) => Some(id.clone()),
                None => {
                    let id = credential_for(
                        &mut merged,
                        connection.username.as_ref(),
                        connection.key_file.as_ref(),
                    );
                    if let Some(id) = &id {
                        credentials.insert(key, id.clone());
                    }
                    id
                }
            };
            merged.connections.push(Connection {
                id: merged.new_connection_id(&connection.name),
                name: connection.name.clone(),
                parent: Some(folder_id.clone()),
                protocol: Protocol::Ssh,
                host: connection.host.clone(),
                icon: None,
                settings: Settings {
                    port: connection.port,
                    credential,
                    forwards: connection.forwards.clone(),
                    ..Settings::default()
                },
            });
        }

        merged.validate()?;
        let added = merged.connections.len() - self.connections.len();
        *self = merged;
        Ok(added)
    }
}

/// Adds a credential for a user name and key file, or none when there is neither.
fn credential_for(
    store: &mut Store,
    username: Option<&String>,
    key_file: Option<&String>,
) -> Option<String> {
    if username.is_none() && key_file.is_none() {
        return None;
    }
    let name = match (username, key_file) {
        (Some(user), Some(_)) => format!("{user} (PuTTY key)"),
        (Some(user), None) => user.clone(),
        (None, _) => "PuTTY key".to_string(),
    };
    let id = store.new_credential_id(&name);
    store.credentials.push(Credential {
        id: id.clone(),
        name,
        username: username.cloned(),
        domain: None,
        kind: if key_file.is_some() {
            CredentialKind::PrivateKey
        } else {
            CredentialKind::Ask
        },
        key_file: key_file.map(Into::into),
    });
    Some(id)
}

/// Why a PuTTY source could not be read, in words for the user.
pub fn unreadable(source: &str, reason: impl std::fmt::Display) -> Error {
    Error::Import(format!("cannot read {source}: {reason}"))
}

#[cfg(test)]
#[path = "putty_tests.rs"]
mod tests;
