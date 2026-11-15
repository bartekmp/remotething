//! The jump hosts on the way to a connection: which ones, in what order, and
//! how to sign in to each without asking.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use remotething_core::{CredentialKind, Store};
use remotething_remote::ssh::{Auth, Endpoint, Hop};
use remotething_secrets::{Secret, SecretStore};

use crate::i18n::fl;

/// One jump host as read from the tree, before its secret is fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    /// The name shown when something about this jump host goes wrong.
    pub name: String,
    pub endpoint: Endpoint,
    pub username: String,
    /// Where the password or passphrase is stored.
    pub credential_id: String,
    /// The private key file, for a key credential; `None` means a password
    /// (or the agent, see `agent`).
    pub key_file: Option<PathBuf>,
    /// Whether to sign in with the keys of a running ssh-agent.
    pub agent: bool,
}

/// A jump host with everything needed to sign in to it.
#[derive(Clone)]
pub struct Ready {
    endpoint: Endpoint,
    username: String,
    key_file: Option<PathBuf>,
    agent: bool,
    secret: Option<Secret>,
}

impl Ready {
    pub fn hop(&self) -> Option<Hop> {
        let auth = match (&self.key_file, &self.secret) {
            _ if self.agent => Auth::Agent {
                username: self.username.clone(),
                socket: None,
            },
            (Some(path), passphrase) => Auth::KeyFile {
                username: self.username.clone(),
                path: path.clone(),
                passphrase: passphrase.clone(),
            },
            (None, Some(password)) => Auth::Password {
                username: self.username.clone(),
                password: password.clone(),
            },
            (None, None) => return None,
        };
        Some(Hop {
            endpoint: self.endpoint.clone(),
            auth,
        })
    }
}

/// The jump hosts of a connection in the order they are connected to: the
/// one reached directly first, the one next to the target last.
///
/// A jump host cannot be asked for a password while connecting, so each needs
/// a credential with a user name and either a stored password or a key file.
pub fn plan(store: &Store, nearest_jump: &str) -> Result<Vec<Planned>, String> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut next = Some(nearest_jump.to_string());
    while let Some(id) = next {
        if !seen.insert(id.clone()) {
            return Err(fl!("jump-loop", name = id));
        }
        let resolved = store.resolve(&id).map_err(|error| error.to_string())?;
        let name = resolved.name.clone();
        let needs_credential = || fl!("jump-needs-credential", name = name.clone());

        let credential = resolved
            .credential
            .as_ref()
            .and_then(|id| store.credentials.iter().find(|c| &c.id == id))
            .ok_or_else(needs_credential)?;
        let username = credential
            .username
            .clone()
            .filter(|username| !username.is_empty())
            .ok_or_else(needs_credential)?;
        let agent = credential.kind == CredentialKind::Agent;
        let key_file = match (credential.kind, &credential.key_file) {
            (CredentialKind::Password | CredentialKind::Agent, _) => None,
            (CredentialKind::PrivateKey, Some(path)) => Some(PathBuf::from(path)),
            (CredentialKind::PrivateKey, None)
            | (CredentialKind::Ask | CredentialKind::Interactive, _) => {
                return Err(needs_credential());
            }
        };

        chain.push(Planned {
            name,
            endpoint: Endpoint {
                host: resolved.host,
                port: resolved.port,
            },
            username,
            credential_id: credential.id.clone(),
            key_file,
            agent,
        });
        next = resolved.jump;
    }
    // Collected from the target outwards; connecting goes the other way.
    chain.reverse();
    Ok(chain)
}

/// Fetches the stored secret of every planned jump host. This can block on a
/// keyring prompt, so it runs off the UI thread.
pub fn load(planned: Vec<Planned>, secrets: Arc<dyn SecretStore>) -> Result<Vec<Ready>, String> {
    planned
        .into_iter()
        .map(|jump| {
            // The agent holds the keys; nothing is stored for it.
            let secret = if jump.agent {
                None
            } else {
                secrets
                    .get(&jump.credential_id)
                    .map_err(|error| error.to_string())?
            };
            // A key file may need no passphrase; a password must be stored.
            if !jump.agent && jump.key_file.is_none() && secret.is_none() {
                return Err(fl!("jump-needs-credential", name = jump.name));
            }
            Ok(Ready {
                endpoint: jump.endpoint,
                username: jump.username,
                key_file: jump.key_file,
                agent: jump.agent,
                secret,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use remotething_secrets::MemoryStore;

    use super::*;

    fn store() -> Store {
        toml::from_str(
            r#"
            [[credential]]
            id = "ops"
            name = "Ops"
            username = "ops"
            kind = "password"

            [[credential]]
            id = "key"
            name = "Key"
            username = "deploy"
            kind = "private-key"
            key_file = "/keys/id_ed25519"

            [[credential]]
            id = "ask"
            name = "Ask"
            username = "ops"
            kind = "ask"

            [[connection]]
            id = "edge"
            name = "edge"
            protocol = "ssh"
            host = "edge.example.net"
            port = 2222
            credential = "ops"

            [[connection]]
            id = "bastion"
            name = "bastion"
            protocol = "ssh"
            host = "bastion.internal"
            credential = "key"
            jump = "edge"

            [[connection]]
            id = "asks"
            name = "asks"
            protocol = "ssh"
            host = "asks.internal"
            credential = "ask"

            [[connection]]
            id = "bare"
            name = "bare"
            protocol = "ssh"
            host = "bare.internal"
            "#,
        )
        .unwrap()
    }

    #[test]
    fn a_chain_is_listed_in_the_order_it_is_connected() {
        let planned = plan(&store(), "bastion").unwrap();

        let hosts: Vec<&str> = planned.iter().map(|j| j.endpoint.host.as_str()).collect();
        assert_eq!(hosts, ["edge.example.net", "bastion.internal"]);
        assert_eq!(planned[0].endpoint.port, 2222);
        assert_eq!(planned[1].key_file, Some(PathBuf::from("/keys/id_ed25519")));
    }

    #[test]
    fn a_jump_host_that_would_have_to_ask_is_refused_by_name() {
        let store = store();

        assert!(plan(&store, "asks").unwrap_err().contains("asks"));
        assert!(plan(&store, "bare").unwrap_err().contains("bare"));
    }

    #[test]
    fn a_loop_of_jump_hosts_is_an_error_not_an_endless_wait() {
        let mut store = store();
        store.connections[0].settings.jump = Some("bastion".to_string());

        assert!(plan(&store, "bastion").is_err());
    }

    #[test]
    fn a_password_jump_host_needs_its_password_stored() {
        let planned = plan(&store(), "edge").unwrap();
        let secrets = Arc::new(MemoryStore::new());

        assert!(load(planned.clone(), secrets.clone()).is_err());

        secrets
            .set("ops", &Secret::new("hunter2".to_string()))
            .unwrap();
        let ready = load(planned, secrets).unwrap();
        assert!(ready[0].hop().is_some());
    }

    #[test]
    fn an_agent_jump_host_needs_no_stored_secret() {
        let mut store = store();
        store.credentials.push(remotething_core::Credential {
            id: "agent".to_string(),
            name: "Agent".to_string(),
            username: Some("ops".to_string()),
            domain: None,
            kind: CredentialKind::Agent,
            key_file: None,
        });
        store.connections[0].settings.credential = Some("agent".to_string());
        let planned = plan(&store, "edge").unwrap();

        let ready = load(planned, Arc::new(MemoryStore::new())).unwrap();

        assert!(matches!(
            ready[0].hop().map(|hop| hop.auth),
            Some(Auth::Agent { .. })
        ));
    }

    #[test]
    fn a_key_jump_host_works_without_a_stored_passphrase() {
        let planned = plan(&store(), "bastion").unwrap();
        let secrets = Arc::new(MemoryStore::new());
        secrets
            .set("ops", &Secret::new("hunter2".to_string()))
            .unwrap();

        let ready = load(planned, secrets).unwrap();

        assert_eq!(ready.len(), 2);
        assert!(ready[1].hop().is_some());
    }

    #[test]
    fn a_keyboard_interactive_jump_host_is_refused_by_name() {
        let mut store = store();
        store.credentials.push(remotething_core::Credential {
            id: "otp".to_string(),
            name: "Otp".to_string(),
            username: Some("ops".to_string()),
            domain: None,
            kind: CredentialKind::Interactive,
            key_file: None,
        });
        store.connections[0].settings.credential = Some("otp".to_string());

        assert!(plan(&store, "edge").unwrap_err().contains("edge"));
    }
}
