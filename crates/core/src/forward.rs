//! Local port forwards: a port on this machine that leads to a host and port
//! as seen from the SSH server.

use std::fmt;

use serde::{Deserialize, Serialize};

/// One local forward, like `ssh -L 8080:db.internal:5432`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forward {
    /// The port listened on, on this machine's loopback address only.
    pub local_port: u16,
    /// The host to reach, as seen from the SSH server.
    pub host: String,
    /// The port on that host.
    pub port: u16,
}

impl fmt::Display for Forward {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}:{}", self.local_port, self.host, self.port)
    }
}

/// Why a forward could not be read. Carries the text that was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadForward(pub String);

impl Forward {
    /// Reads `local_port:host:port`. An IPv6 host goes in brackets.
    pub fn parse(text: &str) -> Result<Forward, BadForward> {
        let bad = || BadForward(text.trim().to_string());
        let text = text.trim();
        let (local, rest) = text.split_once(':').ok_or_else(bad)?;
        let (host, port) = rest.rsplit_once(':').ok_or_else(bad)?;
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let port_of = |text: &str| text.trim().parse::<u16>().ok().filter(|port| *port > 0);
        if host.is_empty() {
            return Err(bad());
        }
        Ok(Forward {
            local_port: port_of(local).ok_or_else(bad)?,
            host: host.to_string(),
            port: port_of(port).ok_or_else(bad)?,
        })
    }
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
