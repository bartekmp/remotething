//! What a folder listing holds, and remote paths as text.

/// What kind of thing a folder entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A folder.
    Folder,
    /// A symbolic link; where it points is not followed in a listing.
    Link,
    /// A device, socket or anything else.
    Other,
}

/// One line of a folder listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The name inside its folder, without the path.
    pub name: String,
    /// File, folder, link or other.
    pub kind: EntryKind,
    /// Size in bytes, when the server reports it.
    pub size: Option<u64>,
    /// Last change, in seconds since 1970, when the server reports it.
    pub modified: Option<u64>,
    /// Unix permission bits such as `0o644`, when the server reports them.
    pub permissions: Option<u32>,
}

/// Orders a listing: folders first, then everything else, each by name without regard to case.
pub fn sort(entries: &mut [Entry]) {
    entries.sort_by_cached_key(|entry| {
        let is_folder = entry.kind == EntryKind::Folder;
        (!is_folder, entry.name.to_lowercase(), entry.name.clone())
    });
}

/// The path of `name` inside `folder`. Remote paths always use `/`.
pub fn join(folder: &str, name: &str) -> String {
    if folder.ends_with('/') {
        format!("{folder}{name}")
    } else {
        format!("{folder}/{name}")
    }
}

/// The folder that holds `path`, or `None` for the top of the file system.
pub fn parent(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.rsplit_once('/') {
        Some(("", _)) => Some("/".to_string()),
        Some((folder, _)) => Some(folder.to_string()),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: EntryKind) -> Entry {
        Entry {
            name: name.to_string(),
            kind,
            size: None,
            modified: None,
            permissions: None,
        }
    }

    #[test]
    fn folders_come_first_and_names_ignore_case() {
        let mut entries = vec![
            entry("zeta.txt", EntryKind::File),
            entry("Beta", EntryKind::Folder),
            entry("alpha.txt", EntryKind::File),
            entry("alpha", EntryKind::Folder),
            entry("Link", EntryKind::Link),
        ];

        sort(&mut entries);

        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["alpha", "Beta", "alpha.txt", "Link", "zeta.txt"]);
    }

    #[test]
    fn joining_adds_exactly_one_slash() {
        assert_eq!(join("/home/ops", "notes.txt"), "/home/ops/notes.txt");
        assert_eq!(join("/", "etc"), "/etc");
    }

    #[test]
    fn the_parent_of_a_path_is_its_folder() {
        assert_eq!(parent("/home/ops").as_deref(), Some("/home"));
        assert_eq!(parent("/home/ops/").as_deref(), Some("/home"));
        assert_eq!(parent("/home").as_deref(), Some("/"));
    }

    #[test]
    fn the_top_has_no_parent() {
        assert_eq!(parent("/"), None);
        assert_eq!(parent(""), None);
    }
}
