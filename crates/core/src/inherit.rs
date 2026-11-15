//! Folder inheritance: a value left unset is taken from the nearest folder that sets it.

use crate::forward::Forward;
use crate::model::{Folder, Protocol, Quality, Settings, Store};
use crate::{Error, Result};

/// A connection with every inherited value filled in, ready to connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConnection {
    /// The connection's id.
    pub id: String,
    /// The label shown in the tree.
    pub name: String,
    /// How to talk to the host.
    pub protocol: Protocol,
    /// Host name or IP address.
    pub host: String,
    /// The explicit, inherited or default port.
    pub port: u16,
    /// Id of the credential to use, if any level sets one.
    pub credential: Option<String>,
    /// Id of the connection to tunnel through, if any level sets one.
    pub jump: Option<String>,
    /// Local port forwards to open with an SSH session.
    pub forwards: Vec<Forward>,
    /// RDP quality preset; `Balanced` when no level sets one.
    pub desktop_quality: Quality,
}

impl Settings {
    /// Fills every field that is still unset from `fallback`.
    pub fn inherit_from(&mut self, fallback: &Settings) {
        if self.port.is_none() {
            self.port = fallback.port;
        }
        if self.credential.is_none() {
            self.credential = fallback.credential.clone();
        }
        if self.jump.is_none() {
            self.jump = fallback.jump.clone();
        }
        if self.forwards.is_empty() {
            self.forwards = fallback.forwards.clone();
        }
        if self.desktop.quality.is_none() {
            self.desktop.quality = fallback.desktop.quality;
        }
    }
}

impl Store {
    /// Returns the connection with all inherited settings applied.
    ///
    /// The connection's own values win, then its folder's, then that folder's
    /// parent, and so on up to the top level.
    pub fn resolve(&self, connection_id: &str) -> Result<ResolvedConnection> {
        let connection = self
            .connections
            .iter()
            .find(|connection| connection.id == connection_id)
            .ok_or_else(|| Error::UnknownConnection(connection_id.to_string()))?;

        let mut settings = connection.settings.clone();
        for folder in self.ancestors(&connection.id, connection.parent.as_deref())? {
            settings.inherit_from(&folder.settings);
        }

        Ok(ResolvedConnection {
            id: connection.id.clone(),
            name: connection.name.clone(),
            protocol: connection.protocol,
            host: connection.host.clone(),
            port: settings.port.unwrap_or(connection.protocol.default_port()),
            credential: settings.credential,
            jump: settings.jump,
            forwards: settings.forwards,
            desktop_quality: settings.desktop.quality.unwrap_or_default(),
        })
    }

    /// The chain of folders above an item, nearest first.
    ///
    /// `item` is only used to name the culprit in errors.
    pub(crate) fn ancestors(&self, item: &str, parent: Option<&str>) -> Result<Vec<&Folder>> {
        let mut chain: Vec<&Folder> = Vec::new();
        let mut next = parent;

        while let Some(folder_id) = next {
            let folder = self
                .folders
                .iter()
                .find(|folder| folder.id == folder_id)
                .ok_or_else(|| Error::UnknownParent {
                    item: item.to_string(),
                    parent: folder_id.to_string(),
                })?;

            if chain.iter().any(|seen| seen.id == folder.id) {
                return Err(Error::FolderCycle(folder.id.clone()));
            }
            chain.push(folder);
            next = folder.parent.as_deref();
        }

        Ok(chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Connection, DesktopSettings};

    fn folder(id: &str, parent: Option<&str>, settings: Settings) -> Folder {
        Folder {
            id: id.to_string(),
            name: id.to_string(),
            parent: parent.map(str::to_string),
            icon: None,
            settings,
        }
    }

    fn connection(id: &str, parent: Option<&str>, settings: Settings) -> Connection {
        Connection {
            id: id.to_string(),
            name: id.to_string(),
            parent: parent.map(str::to_string),
            protocol: Protocol::Ssh,
            host: format!("{id}.example.net"),
            icon: None,
            settings,
        }
    }

    fn credential(id: &str) -> Settings {
        Settings {
            credential: Some(id.to_string()),
            ..Settings::default()
        }
    }

    #[test]
    fn a_top_level_connection_gets_protocol_defaults() {
        let store = Store {
            credentials: vec![],
            folders: vec![],
            connections: vec![connection("web", None, Settings::default())],
        };

        let resolved = store.resolve("web").unwrap();

        assert_eq!(resolved.port, 22);
        assert_eq!(resolved.credential, None);
        assert_eq!(resolved.desktop_quality, Quality::Balanced);
    }

    #[test]
    fn an_unset_value_comes_from_the_folder() {
        let store = Store {
            credentials: vec![],
            folders: vec![folder("prod", None, credential("cred-ops"))],
            connections: vec![connection("web", Some("prod"), Settings::default())],
        };

        let resolved = store.resolve("web").unwrap();

        assert_eq!(resolved.credential.as_deref(), Some("cred-ops"));
    }

    #[test]
    fn the_connections_own_value_wins_over_the_folder() {
        let store = Store {
            credentials: vec![],
            folders: vec![folder("prod", None, credential("cred-ops"))],
            connections: vec![connection("web", Some("prod"), credential("cred-web"))],
        };

        let resolved = store.resolve("web").unwrap();

        assert_eq!(resolved.credential.as_deref(), Some("cred-web"));
    }

    #[test]
    fn the_nearest_folder_wins_over_a_farther_one() {
        let outer = Settings {
            port: Some(2222),
            ..credential("cred-outer")
        };
        let store = Store {
            credentials: vec![],
            folders: vec![
                folder("outer", None, outer),
                folder("inner", Some("outer"), credential("cred-inner")),
            ],
            connections: vec![connection("web", Some("inner"), Settings::default())],
        };

        let resolved = store.resolve("web").unwrap();

        assert_eq!(resolved.credential.as_deref(), Some("cred-inner"));
        // The inner folder leaves the port unset, so the outer one supplies it.
        assert_eq!(resolved.port, 2222);
    }

    #[test]
    fn nested_rdp_settings_are_inherited_too() {
        let low = Settings {
            desktop: DesktopSettings {
                quality: Some(Quality::LowBandwidth),
            },
            ..Settings::default()
        };
        let store = Store {
            credentials: vec![],
            folders: vec![folder("remote-sites", None, low)],
            connections: vec![connection("pc", Some("remote-sites"), Settings::default())],
        };

        let resolved = store.resolve("pc").unwrap();

        assert_eq!(resolved.desktop_quality, Quality::LowBandwidth);
    }

    #[test]
    fn an_unknown_connection_is_an_error() {
        let error = Store::default().resolve("ghost").unwrap_err();

        assert!(matches!(error, Error::UnknownConnection(id) if id == "ghost"));
    }

    #[test]
    fn a_folder_cycle_is_an_error_not_an_endless_loop() {
        let store = Store {
            credentials: vec![],
            folders: vec![
                folder("a", Some("b"), Settings::default()),
                folder("b", Some("a"), Settings::default()),
            ],
            connections: vec![connection("web", Some("a"), Settings::default())],
        };

        let error = store.resolve("web").unwrap_err();

        assert!(matches!(error, Error::FolderCycle(_)));
    }

    #[test]
    fn forwards_come_from_the_folder_unless_the_connection_has_its_own() {
        let forward = |local_port| crate::Forward {
            local_port,
            host: "db".to_string(),
            port: 5432,
        };
        let with = |local_port| Settings {
            forwards: vec![forward(local_port)],
            ..Settings::default()
        };
        let store = Store {
            folders: vec![folder("f", None, with(1000))],
            connections: vec![
                connection("plain", Some("f"), Settings::default()),
                connection("own", Some("f"), with(2000)),
            ],
            ..Store::default()
        };

        assert_eq!(store.resolve("plain").unwrap().forwards, [forward(1000)]);
        assert_eq!(store.resolve("own").unwrap().forwards, [forward(2000)]);
    }
}
