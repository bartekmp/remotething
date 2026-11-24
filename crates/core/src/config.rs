//! Reading and writing the connections file.

use std::fs::{self, File};
use std::io::Write as _;
use std::path::Path;

use crate::model::Store;
use crate::{Error, Result};

impl Store {
    /// Reads and validates the connections file.
    pub fn load(path: &Path) -> Result<Store> {
        let text = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
        let store: Store = toml::from_str(&text).map_err(|source| Error::Parse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
        store.validate()?;
        Ok(store)
    }

    /// Writes the connections file without ever leaving a half-written one.
    ///
    /// The new content goes to a temporary file beside the target, is flushed
    /// to disk, and then replaces the target in one rename.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let text = toml::to_string_pretty(self)?;

        let mut temporary = path.as_os_str().to_owned();
        temporary.push(".tmp");
        let temporary = Path::new(&temporary);

        let write = || -> std::io::Result<()> {
            let mut file = create_private(temporary)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            fs::rename(temporary, path)
        };
        write().map_err(|source| io_error(path, source))
    }
}

/// On Unix the file is for its owner alone: it lists hosts and user names.
#[cfg(unix)]
fn create_private(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;

    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private(path: &Path) -> std::io::Result<File> {
    File::create(path)
}

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CredentialKind, Protocol, Quality};

    /// The example from the development plan.
    const PLAN_EXAMPLE: &str = r#"
[[credential]]
id = "cred-ops"
name = "Ops team"
username = "ops"
kind = "private-key"
key_file = "~/.ssh/id_ops"

[[folder]]
id = "prod"
name = "Production"
icon = "building"
credential = "cred-ops"        # inherited by children
rdp.quality = "balanced"

[[connection]]
id = "bastion"
name = "bastion"
protocol = "ssh"
host = "bastion.example.net"

[[connection]]
id = "web-01"
parent = "prod"
name = "web-01"
protocol = "ssh"
host = "web-01.example.net"
jump = "bastion"               # another connection's id
"#;

    #[test]
    fn the_plan_example_loads_and_resolves() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        fs::write(&path, PLAN_EXAMPLE).unwrap();

        let store = Store::load(&path).unwrap();
        let web = store.resolve("web-01").unwrap();

        assert_eq!(
            store.folders[0].settings.desktop.quality,
            Some(Quality::Balanced)
        );
        assert_eq!(web.protocol, Protocol::Ssh);
        assert_eq!(web.credential.as_deref(), Some("cred-ops"));
        assert_eq!(web.jump.as_deref(), Some("bastion"));
        assert_eq!(store.credentials[0].kind, CredentialKind::PrivateKey);
        assert_eq!(store.folders[0].icon.as_deref(), Some("building"));
        assert_eq!(store.connections[0].icon, None);
    }

    #[test]
    fn an_sftp_connection_loads_and_gets_the_ssh_port() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        fs::write(
            &path,
            "[[connection]]\nid = \"files\"\nname = \"files\"\nprotocol = \"sftp\"\nhost = \"nas\"\n",
        )
        .unwrap();

        let store = Store::load(&path).unwrap();
        let resolved = store.resolve("files").unwrap();

        assert_eq!(resolved.protocol, Protocol::Sftp);
        assert_eq!(resolved.port, 22);
        assert!(resolved.protocol.uses_ssh());
        assert!(!resolved.protocol.is_desktop());
    }

    #[test]
    fn the_quality_loads_under_its_new_name_and_is_saved_under_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        fs::write(
            &path,
            "[[folder]]\nid = \"f\"\nname = \"F\"\ndesktop.quality = \"lan\"\n",
        )
        .unwrap();

        let store = Store::load(&path).unwrap();
        store.save(&path).unwrap();

        assert_eq!(
            store.folders[0].settings.desktop.quality,
            Some(Quality::Lan)
        );
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("quality"), "{saved}");
        assert!(!saved.contains("rdp"), "{saved}");
    }

    #[cfg(unix)]
    #[test]
    fn the_saved_file_is_for_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        let store: Store = toml::from_str(PLAN_EXAMPLE).unwrap();

        store.save(&path).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn save_then_load_gives_back_the_same_tree() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        let original: Store = toml::from_str(PLAN_EXAMPLE).unwrap();

        original.save(&path).unwrap();
        let reloaded = Store::load(&path).unwrap();

        assert_eq!(reloaded, original);
    }

    #[test]
    fn save_leaves_no_temporary_file_behind() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");

        Store::default().save(&path).unwrap();

        let names: Vec<_> = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["connections.toml"]);
    }

    #[test]
    fn unset_values_are_not_written_to_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        let store: Store = toml::from_str(PLAN_EXAMPLE).unwrap();

        store.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();

        assert!(!text.contains("port"));
        // Only the folder sets an RDP value.
        assert_eq!(text.matches("quality").count(), 1);
    }

    #[test]
    fn an_invalid_tree_is_not_saved() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        let mut store: Store = toml::from_str(PLAN_EXAMPLE).unwrap();
        store.connections[1].settings.jump = Some("nobody".to_string());

        assert!(matches!(store.save(&path), Err(Error::UnknownJump { .. })));
        assert!(!path.exists());
    }

    #[test]
    fn a_missing_file_reports_its_path() {
        let error = Store::load(Path::new("/nonexistent/connections.toml")).unwrap_err();

        assert!(error.to_string().contains("/nonexistent/connections.toml"));
    }

    #[test]
    fn broken_toml_is_a_parse_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("connections.toml");
        fs::write(&path, "[[connection]]\nid = ").unwrap();

        assert!(matches!(Store::load(&path), Err(Error::Parse { .. })));
    }
}
