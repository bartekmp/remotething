//! Host certificates: a server proves who it is with a certificate signed by an authority
//! the user trusts, instead of a key accepted one host at a time.
//!
//! The authorities are the `@cert-authority` lines of OpenSSH's `~/.ssh/known_hosts`,
//! so a fleet that already works with `ssh` works here without another setup.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use remotething_secrets::KnownHosts;
use russh::keys::ssh_key::certificate::CertType;
use russh::keys::{Certificate, HashAlg, PublicKey};

use super::{Endpoint, Error};

/// A certificate authority and the hosts it may vouch for.
pub(super) struct Authority {
    patterns: Vec<String>,
    key: PublicKey,
}

/// The `@cert-authority` lines of the user's OpenSSH `known_hosts` (or the file `known`
/// names instead), if there are any.
pub(super) fn load(known: &KnownHosts) -> Vec<Authority> {
    let path = match known.openssh_file() {
        Some(path) => path.to_path_buf(),
        None => {
            let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
            else {
                return Vec::new();
            };
            PathBuf::from(home).join(".ssh").join("known_hosts")
        }
    };
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(_) => Vec::new(),
    }
}

/// Reads `@cert-authority patterns keytype key [comment]` lines; others are ignored.
pub(super) fn parse(text: &str) -> Vec<Authority> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            if words.next()? != "@cert-authority" {
                return None;
            }
            let patterns = words.next()?;
            let key = format!("{} {}", words.next()?, words.next()?);
            Some(Authority {
                patterns: patterns.split(',').map(str::to_string).collect(),
                key: PublicKey::from_openssh(&key).ok()?,
            })
        })
        .collect()
}

impl Authority {
    /// Whether the patterns include this host: `*` and `?` match, and a `!` pattern
    /// excludes. A port other than 22 is written `[host]:port`, as in OpenSSH.
    fn covers(&self, endpoint: &Endpoint) -> bool {
        let name = if endpoint.port == 22 {
            endpoint.host.clone()
        } else {
            format!("[{}]:{}", endpoint.host, endpoint.port)
        };
        let mut covered = false;
        for pattern in &self.patterns {
            match pattern.strip_prefix('!') {
                Some(excluded) if matches(excluded, &name) => return false,
                Some(_) => {}
                None => covered |= matches(pattern, &name),
            }
        }
        covered
    }
}

/// Whether any authority vouches for this host, which is when asking the server for a
/// certificate is worthwhile.
pub(super) fn any_covers(authorities: &[Authority], endpoint: &Endpoint) -> bool {
    authorities
        .iter()
        .any(|authority| authority.covers(endpoint))
}

/// Accepts a certificate for this host signed by one of its authorities.
pub(super) fn check(
    authorities: &[Authority],
    endpoint: &Endpoint,
    certificate: &Certificate,
) -> Result<(), Error> {
    let refuse = |reason: &str| Err(Error::Protocol(format!("host certificate: {reason}")));
    if certificate.cert_type() != CertType::Host {
        return refuse("it is a user certificate");
    }
    // No principals means any host; otherwise the host must be one of them.
    let principals = certificate.valid_principals();
    if !principals.is_empty() && !principals.iter().any(|name| matches(name, &endpoint.host)) {
        return refuse(&format!("it is not valid for {}", endpoint.host));
    }
    // Options this program does not understand must refuse the certificate.
    if !certificate.critical_options().is_empty() {
        return refuse("it has options this program does not understand");
    }
    let fingerprints: Vec<_> = authorities
        .iter()
        .filter(|authority| authority.covers(endpoint))
        .map(|authority| authority.key.fingerprint(HashAlg::Sha256))
        .collect();
    if fingerprints.is_empty() {
        return refuse("no trusted authority covers this host");
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    match certificate.validate_at(now, &fingerprints) {
        Ok(()) => Ok(()),
        Err(_) => {
            refuse("it is expired, badly signed, or signed by an authority that is not trusted")
        }
    }
}

/// Glob matching with `*` for any run of characters and `?` for one, ignoring case.
fn matches(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let name: Vec<char> = name.to_lowercase().chars().collect();
    let (mut p, mut n) = (0, 0);
    // Where the last `*` was, to try it longer when a later part fails.
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, n));
            p += 1;
        } else if let Some((star_at, name_at)) = star {
            p = star_at + 1;
            n = name_at + 1;
            star = Some((star_at, name_at + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
#[path = "host_certificate_tests.rs"]
mod tests;
