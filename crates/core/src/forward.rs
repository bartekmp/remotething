//! Local port forwards: a port on this machine that leads to a host and port
//! as seen from the SSH server.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Which way a forward carries connections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForwardKind {
    /// Listens here and reaches a host as seen from the server, like `ssh -L`.
    #[default]
    Local,
    /// Listens on the server and reaches a host as seen from here, like `ssh -R`.
    Remote,
    /// Listens here as a SOCKS5 proxy that reaches anything the server can, like `ssh -D`.
    Dynamic,
}

impl ForwardKind {
    fn is_local(&self) -> bool {
        *self == ForwardKind::Local
    }
}

/// One port forward.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forward {
    /// Which way connections go.
    #[serde(default, skip_serializing_if = "ForwardKind::is_local")]
    pub kind: ForwardKind,
    /// The port listened on: on this machine's loopback address for local and dynamic
    /// forwards, on the server for a remote one.
    pub local_port: u16,
    /// The host to reach. Empty for a dynamic forward.
    pub host: String,
    /// The port on that host. 0 for a dynamic forward.
    pub port: u16,
}

impl fmt::Display for Forward {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ForwardKind::Local => {
                write!(formatter, "{}:{}:{}", self.local_port, self.host, self.port)
            }
            ForwardKind::Remote => {
                write!(
                    formatter,
                    "R:{}:{}:{}",
                    self.local_port, self.host, self.port
                )
            }
            ForwardKind::Dynamic => write!(formatter, "D:{}", self.local_port),
        }
    }
}

/// Why a forward could not be read. Carries the text that was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadForward(pub String);

impl Forward {
    /// Reads `local_port:host:port` (local), `R:port:host:port` (remote) or `D:port` (dynamic).
    /// An IPv6 host goes in brackets.
    pub fn parse(text: &str) -> Result<Forward, BadForward> {
        let bad = || BadForward(text.trim().to_string());
        let text = text.trim();
        if let Some(port) = text.strip_prefix("D:").or_else(|| text.strip_prefix("d:")) {
            let port = port_of(port).ok_or_else(bad)?;
            return Ok(Forward {
                kind: ForwardKind::Dynamic,
                local_port: port,
                host: String::new(),
                port: 0,
            });
        }
        let (kind, text) = match text.strip_prefix("R:").or_else(|| text.strip_prefix("r:")) {
            Some(rest) => (ForwardKind::Remote, rest),
            None => (ForwardKind::Local, text),
        };
        let (local, rest) = text.split_once(':').ok_or_else(bad)?;
        let (host, port) = rest.rsplit_once(':').ok_or_else(bad)?;
        let host = host.trim_start_matches('[').trim_end_matches(']');
        if host.is_empty() {
            return Err(bad());
        }
        Ok(Forward {
            kind,
            local_port: port_of(local).ok_or_else(bad)?,
            host: host.to_string(),
            port: port_of(port).ok_or_else(bad)?,
        })
    }
}

fn port_of(text: &str) -> Option<u16> {
    text.trim().parse::<u16>().ok().filter(|port| *port > 0)
}

/// Reads a comma-separated list. Empty text is an empty list.
pub fn parse_forwards(text: &str) -> Result<Vec<Forward>, BadForward> {
    text.split(',')
        .filter(|part| !part.trim().is_empty())
        .map(Forward::parse)
        .collect()
}

/// The inverse of [`parse_forwards`], for showing in a text field.
pub fn format_forwards(forwards: &[Forward]) -> String {
    let parts: Vec<String> = forwards.iter().map(Forward::to_string).collect();
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forward(local_port: u16, host: &str, port: u16) -> Forward {
        Forward {
            kind: ForwardKind::Local,
            local_port,
            host: host.to_string(),
            port,
        }
    }

    #[test]
    fn a_forward_is_three_parts() {
        assert_eq!(
            Forward::parse("8080:db.internal:5432"),
            Ok(forward(8080, "db.internal", 5432))
        );
    }

    #[test]
    fn a_list_skips_blanks_and_keeps_order() {
        let list = parse_forwards("1:a:2, 3:b:4,").unwrap();

        assert_eq!(list, [forward(1, "a", 2), forward(3, "b", 4)]);
        assert_eq!(parse_forwards("  ").unwrap(), []);
    }

    #[test]
    fn a_bracketed_ipv6_host_is_unwrapped() {
        assert_eq!(
            Forward::parse("80:[::1]:8080"),
            Ok(forward(80, "::1", 8080))
        );
    }

    #[test]
    fn bad_forwards_name_the_text() {
        for text in [
            "8080",
            "8080:host",
            "x:host:1",
            "0:host:1",
            "1:host:99999",
            "1::2",
        ] {
            assert_eq!(Forward::parse(text), Err(BadForward(text.to_string())));
        }
    }

    #[test]
    fn remote_and_dynamic_forwards_have_prefixes() {
        let remote = Forward::parse("R:9000:localhost:3000").unwrap();
        let dynamic = Forward::parse("D:1080").unwrap();

        assert_eq!(remote.kind, ForwardKind::Remote);
        assert_eq!(
            (remote.local_port, remote.host.as_str(), remote.port),
            (9000, "localhost", 3000)
        );
        assert_eq!(dynamic.kind, ForwardKind::Dynamic);
        assert_eq!(dynamic.local_port, 1080);
        assert_eq!(remote.to_string(), "R:9000:localhost:3000");
        assert_eq!(dynamic.to_string(), "D:1080");
        assert!(Forward::parse("D:").is_err());
        assert!(Forward::parse("D:1080:x:2").is_err());
        assert!(Forward::parse("R:9000").is_err());
    }

    #[test]
    fn the_file_keeps_all_kinds() {
        let list = parse_forwards("1:a:2, R:3:b:4, D:5").unwrap();
        let text = toml::to_string(&crate::Settings {
            forwards: list.clone(),
            ..crate::Settings::default()
        })
        .unwrap();
        let back: crate::Settings = toml::from_str(&text).unwrap();

        assert_eq!(back.forwards, list);
        assert!(!text.contains("kind = \"local\""));
    }

    #[test]
    fn formatting_round_trips() {
        let list = vec![forward(1, "a", 2), forward(3, "b.c", 4)];

        assert_eq!(parse_forwards(&format_forwards(&list)).unwrap(), list);
    }

    #[test]
    fn forwards_survive_the_connections_file() {
        let mut store = crate::Store::default();
        store.connections.push(crate::Connection {
            id: "web".to_string(),
            name: "web".to_string(),
            parent: None,
            protocol: crate::Protocol::Ssh,
            host: "web.example.net".to_string(),
            icon: None,
            settings: crate::Settings {
                forwards: vec![forward(8080, "db", 5432), forward(9000, "::1", 9000)],
                ..crate::Settings::default()
            },
        });

        let text = toml::to_string(&store).unwrap();
        let back: crate::Store = toml::from_str(&text).unwrap();

        assert_eq!(back, store);
    }
}
