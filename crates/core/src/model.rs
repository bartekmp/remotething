use std::path::PathBuf;

use crate::forward::Forward;
use serde::{Deserialize, Serialize};

/// The whole connection tree, exactly as stored in the connections file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Store {
    /// Credential descriptions, in file order. The secrets live elsewhere.
    #[serde(rename = "credential", default, skip_serializing_if = "Vec::is_empty")]
    pub credentials: Vec<Credential>,
    /// Folders, in file order.
    #[serde(rename = "folder", default, skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<Folder>,
    /// Connections, in file order.
    #[serde(rename = "connection", default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<Connection>,
}

/// Who to log in as, and where the secret comes from.
///
/// The password or passphrase itself is kept in the secret store under `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    /// Unique among credentials; also the key in the secret store.
    pub id: String,
    /// The label shown in the credential list.
    pub name: String,
    /// Login name, when the credential fixes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Windows domain, for RDP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// What kind of secret this is.
    pub kind: CredentialKind,
    /// Private key file, for `private-key` credentials that keep the key on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_file: Option<PathBuf>,
}

/// The kinds of credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialKind {
    /// A password kept in the secret store.
    Password,
    /// A private key; its passphrase, if any, is kept in the secret store.
    PrivateKey,
    /// Nothing is stored; the user is asked on every connect.
    Ask,
    /// Keys held by a running ssh-agent (on Windows the OpenSSH agent or Pageant).
    /// Only the user name is kept here.
    Agent,
    /// The server asks questions at sign-in (password, one-time code).
    /// Only the user name is kept here.
    Interactive,
}

impl CredentialKind {
    /// Whether a secret is kept in the secret store for this kind.
    pub fn stores_secret(self) -> bool {
        matches!(self, CredentialKind::Password | CredentialKind::PrivateKey)
    }
}

/// A folder in the tree. Its settings are inherited by everything inside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Unique among folders.
    pub id: String,
    /// The label shown in the tree.
    pub name: String,
    /// The enclosing folder, or `None` at the top level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Name of the icon shown in the tree. `None` means the default folder icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Values passed down to children that leave them unset.
    #[serde(flatten)]
    pub settings: Settings,
}

/// One remote machine the user can connect to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    /// Unique among connections.
    pub id: String,
    /// The label shown in the tree.
    pub name: String,
    /// The enclosing folder, or `None` at the top level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// How to talk to the host.
    pub protocol: Protocol,
    /// Host name or IP address.
    pub host: String,
    /// Name of the icon shown in the tree. `None` means one that fits the protocol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Values set here win over anything inherited.
    #[serde(flatten)]
    pub settings: Settings,
}

/// Settings that can live on a folder or a connection. `None` means "inherit".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// TCP port. Falls back to the protocol's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Id of a credential in the secret store. Never the secret itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
    /// Id of the connection to tunnel through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump: Option<String>,
    /// Local port forwards opened with an SSH session. Empty means "inherit".
    #[serde(default, rename = "forward", skip_serializing_if = "Vec::is_empty")]
    pub forwards: Vec<Forward>,
    /// Settings that only apply to RDP.
    #[serde(default, skip_serializing_if = "RdpSettings::is_empty")]
    pub rdp: RdpSettings,
}

/// RDP-only settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RdpSettings {
    /// Bandwidth and quality preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<Quality>,
}

impl RdpSettings {
    /// True when nothing is set, so the table can be left out of the file.
    pub fn is_empty(&self) -> bool {
        self.quality.is_none()
    }
}

/// The protocols a connection can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// Terminal over SSH.
    Ssh,
    /// Windows Remote Desktop.
    Rdp,
    /// VNC remote framebuffer.
    Vnc,
}

impl Protocol {
    /// The port used when neither the connection nor its folders set one.
    pub fn default_port(self) -> u16 {
        match self {
            Protocol::Ssh => 22,
            Protocol::Rdp => 3389,
            Protocol::Vnc => 5900,
        }
    }
}

/// Remote desktop quality presets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Quality {
    /// Best picture, for a local network.
    Lan,
    /// The default trade-off.
    #[default]
    Balanced,
    /// Lowest traffic, for slow links.
    LowBandwidth,
}
