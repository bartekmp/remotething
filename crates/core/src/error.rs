use std::path::PathBuf;

/// Everything that can go wrong in the core crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The connections file could not be read or written.
    #[error("cannot access {path}: {source}")]
    Io {
        /// The file involved.
        path: PathBuf,
        /// The underlying operating system error.
        source: std::io::Error,
    },

    /// The connections file is not valid TOML or has the wrong shape.
    #[error("cannot parse {path}: {source}")]
    Parse {
        /// The file involved.
        path: PathBuf,
        /// Where and why parsing stopped.
        source: Box<toml::de::Error>,
    },

    /// The tree could not be turned into TOML.
    #[error("cannot serialise connections: {0}")]
    Serialise(#[from] toml::ser::Error),

    /// Two folders, two connections or two credentials share an id.
    #[error("id \"{0}\" is used more than once")]
    DuplicateId(String),

    /// A `parent` names a folder that does not exist.
    #[error("\"{item}\" has unknown parent folder \"{parent}\"")]
    UnknownParent {
        /// The folder or connection with the bad reference.
        item: String,
        /// The missing folder id.
        parent: String,
    },

    /// A `credential` names a credential that does not exist.
    #[error("\"{item}\" has unknown credential \"{credential}\"")]
    UnknownCredential {
        /// The folder or connection with the bad reference.
        item: String,
        /// The missing credential id.
        credential: String,
    },

    /// A `jump` names a connection that does not exist.
    #[error("\"{item}\" has unknown jump host \"{jump}\"")]
    UnknownJump {
        /// The folder or connection with the bad reference.
        item: String,
        /// The missing connection id.
        jump: String,
    },

    /// A folder is, directly or not, its own parent.
    #[error("folder \"{0}\" is inside itself")]
    FolderCycle(String),

    /// A folder that still has folders or connections inside cannot be removed.
    #[error("folder \"{0}\" is not empty")]
    FolderNotEmpty(String),

    /// A credential that a folder or connection uses cannot be removed.
    #[error("credential \"{credential}\" is used by \"{user}\"")]
    CredentialInUse {
        /// The credential that was to be removed.
        credential: String,
        /// One folder or connection that still names it.
        user: String,
    },

    /// A connection that others tunnel through cannot be removed.
    #[error("\"{jump}\" is the jump host of \"{user}\"")]
    JumpInUse {
        /// The connection that was to be removed.
        jump: String,
        /// One folder or connection that still names it.
        user: String,
    },

    /// The tree could not be turned into an export file.
    #[error("cannot export the connections: {0}")]
    Export(String),

    /// The file is not an export this version can read.
    #[error("cannot import this file: {0}")]
    Import(String),

    /// No connection has this id.
    #[error("no connection with id \"{0}\"")]
    UnknownConnection(String),
}
