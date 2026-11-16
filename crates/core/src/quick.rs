//! Connection strings typed by hand, such as `ssh ops@web-01` or `rdp://desk:3390`.

use crate::model::Protocol;

/// Where a typed connection string points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickTarget {
    /// The protocol named in the string; SSH for a bare `user@host`.
    pub protocol: Protocol,
    /// The user in front of the `@`, if there is one.
    pub username: Option<String>,
    /// Host name or IP address.
    pub host: String,
    /// The port, when the string names one.
    pub port: Option<u16>,
}

impl QuickTarget {
    /// The named port, or the usual one for the protocol.
    pub fn port_or_default(&self) -> u16 {
        self.port.unwrap_or(self.protocol.default_port())
    }

    /// A short label such as `ops@web-01`.
    pub fn label(&self) -> String {
        match &self.username {
            Some(username) => format!("{username}@{}", self.host),
            None => self.host.clone(),
        }
    }
}

/// Reads a connection string. `None` when the text is not one, for example a plain search word.
///
/// Understood forms:
/// - `ssh user@host`, `ssh host`, with `-p 2222` anywhere after `ssh`
/// - `rdp host`, `vnc host`
/// - `ssh://user@host:2222`, `sftp://user@host`, `ftp://host`, `ftps://host`, `rdp://host`, `vnc://host:5901`
/// - `user@host` and `user@host:2222`, taken as SSH
/// - an IPv6 address goes in brackets when a port follows: `ssh://[::1]:2222`
pub fn parse_quick(text: &str) -> Option<QuickTarget> {
    let text = text.trim();
    if let Some((scheme, rest)) = text.split_once("://") {
        let protocol = protocol_named(scheme)?;
        return target(protocol, rest.trim_end_matches('/'), None);
    }

    let mut words = text.split_whitespace();
    let first = words.next()?;
    let Some(protocol) = protocol_named(first) else {
        // No protocol word: only `user@host` on its own is a connection string.
        if words.next().is_some() || !first.contains('@') {
            return None;
        }
        return target(Protocol::Ssh, first, None);
    };

    let mut address = None;
    let mut port = None;
    while let Some(word) = words.next() {
        if word == "-p" {
            port = Some(words.next()?.parse::<u16>().ok()?);
        } else if address.is_none() && !word.starts_with('-') {
            address = Some(word);
        } else {
            // Options we do not know would be silently dropped; refuse instead.
            return None;
        }
    }
    target(protocol, address?, port)
}

fn protocol_named(word: &str) -> Option<Protocol> {
    match word.to_ascii_lowercase().as_str() {
        "ssh" => Some(Protocol::Ssh),
        "sftp" => Some(Protocol::Sftp),
        "wsl" => Some(Protocol::Wsl),
        "ftp" => Some(Protocol::Ftp),
        "ftps" => Some(Protocol::Ftps),
        "rdp" => Some(Protocol::Rdp),
        "vnc" => Some(Protocol::Vnc),
        _ => None,
    }
}

/// Splits `user@host:port`. A port given with `-p` wins over one in the address.
fn target(protocol: Protocol, address: &str, flag_port: Option<u16>) -> Option<QuickTarget> {
    let (username, host_and_port) = match address.rsplit_once('@') {
        Some((username, rest)) => (Some(username), rest),
        None => (None, address),
    };
    if username.is_some_and(str::is_empty) {
        return None;
    }

    let (host, port) = if let Some(bracketed) = host_and_port.strip_prefix('[') {
        let (host, after) = bracketed.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(port) => Some(port.parse::<u16>().ok()?),
            None if after.is_empty() => None,
            None => return None,
        };
        (host, port)
    } else {
        match host_and_port.rsplit_once(':') {
            // More than one colon without brackets is a bare IPv6 address.
            Some((host, _)) if host.contains(':') => (host_and_port, None),
            Some((host, port)) => (host, Some(port.parse::<u16>().ok()?)),
            None => (host_and_port, None),
        }
    };
    let port = flag_port.or(port);
    if host.is_empty() || host.contains(char::is_whitespace) || port == Some(0) {
        return None;
    }

    Some(QuickTarget {
        protocol,
        username: username.map(str::to_string),
        host: host.to_string(),
        port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> (Protocol, Option<String>, String, Option<u16>) {
        let target = parse_quick(text).unwrap_or_else(|| panic!("{text:?} should parse"));
        (target.protocol, target.username, target.host, target.port)
    }

    fn user(name: &str) -> Option<String> {
        Some(name.to_string())
    }

    #[test]
    fn the_ssh_command_form_gives_user_and_host() {
        assert_eq!(
            parsed("ssh user@192.168.0.1"),
            (Protocol::Ssh, user("user"), "192.168.0.1".to_string(), None)
        );
        assert_eq!(
            parsed("  ssh web-01  "),
            (Protocol::Ssh, None, "web-01".to_string(), None)
        );
    }

    #[test]
    fn the_sftp_scheme_opens_files_on_the_ssh_port() {
        let target = parse_quick("sftp://ops@web-01").unwrap();

        assert_eq!(target.protocol, Protocol::Sftp);
        assert_eq!(target.username.as_deref(), Some("ops"));
        assert_eq!(target.port_or_default(), 22);
    }

    #[test]
    fn ftp_and_ftps_schemes_use_port_21() {
        for (text, protocol) in [("ftp://nas", Protocol::Ftp), ("ftps://nas", Protocol::Ftps)] {
            let target = parse_quick(text).unwrap();

            assert_eq!(target.protocol, protocol);
            assert_eq!(target.port_or_default(), 21);
        }
    }

    #[test]
    fn the_port_flag_works_before_and_after_the_address() {
        assert_eq!(parsed("ssh -p 2222 ops@web-01").3, Some(2222));
        assert_eq!(parsed("ssh ops@web-01 -p 2222").3, Some(2222));
    }

    #[test]
    fn urls_name_the_protocol_and_may_carry_a_port() {
        assert_eq!(
            parsed("rdp://desk.example.net:3390"),
            (
                Protocol::Rdp,
                None,
                "desk.example.net".to_string(),
                Some(3390)
            )
        );
        assert_eq!(
            parsed("ssh://ops@web-01/"),
            (Protocol::Ssh, user("ops"), "web-01".to_string(), None)
        );
        assert_eq!(parsed("VNC://tv").0, Protocol::Vnc);
    }

    #[test]
    fn a_bare_user_at_host_is_ssh() {
        assert_eq!(
            parsed("ops@web-01:2200"),
            (Protocol::Ssh, user("ops"), "web-01".to_string(), Some(2200))
        );
    }

    #[test]
    fn ipv6_addresses_work_bare_and_in_brackets() {
        assert_eq!(parsed("ssh ops@fe80::1").2, "fe80::1");
        assert_eq!(
            parsed("ssh://[::1]:2222"),
            (Protocol::Ssh, None, "::1".to_string(), Some(2222))
        );
    }

    #[test]
    fn search_words_are_not_connection_strings() {
        for text in ["", "web", "web 01", "ssh", "sshd-box", "prod db"] {
            assert_eq!(parse_quick(text), None, "{text:?}");
        }
    }

    #[test]
    fn broken_strings_are_refused_rather_than_guessed() {
        for text in [
            "ssh @host",
            "ssh host:notaport",
            "ssh host:0",
            "ssh -p",
            "ssh -X host",
            "ssh host extra",
            "gopher://host",
            "ssh://",
        ] {
            assert_eq!(parse_quick(text), None, "{text:?}");
        }
    }

    #[test]
    fn the_default_port_follows_the_protocol_and_the_label_shows_the_user() {
        let target = parse_quick("rdp://desk").unwrap();
        assert_eq!(target.port_or_default(), 3389);

        assert_eq!(parse_quick("ssh ops@web-01").unwrap().label(), "ops@web-01");
    }
}
