//! Turning a folder into the list of file transfers it stands for.

use std::io;
use std::path::{Component, Path, PathBuf};

use crate::i18n::fl;

/// Every file under a local folder, as paths relative to it. Links and
/// special files are left out, so a loop cannot trap the walk.
fn local_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut waiting = vec![PathBuf::new()];
    while let Some(relative) = waiting.pop() {
        for item in std::fs::read_dir(root.join(&relative))? {
            let item = item?;
            let path = relative.join(item.file_name());
            let kind = item.file_type()?;
            if kind.is_dir() {
                waiting.push(path);
            } else if kind.is_file() {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// The uploads of a local folder into `remote_folder`: local file and remote path.
pub(crate) fn uploads(
    local_folder: &Path,
    remote_folder: &str,
) -> io::Result<Vec<(PathBuf, String)>> {
    let uploads = local_files(local_folder)?
        .into_iter()
        .map(|relative| {
            let parts: Vec<_> = relative
                .iter()
                .map(|part| part.to_string_lossy().into_owned())
                .collect();
            let remote = remotething_files::join(remote_folder, &parts.join("/"));
            (local_folder.join(&relative), remote)
        })
        .collect();
    Ok(uploads)
}

/// The downloads of a remote folder into `local_folder`: local file and remote path.
/// `relative_files` are the paths under the remote folder, such as `a/b.txt`.
///
/// The names come from the server, so each part must be a plain name. One that
/// is `..`, empty or a path of its own would write outside `local_folder`, and the
/// whole folder is refused.
pub(crate) fn downloads(
    remote_folder: &str,
    local_folder: &Path,
    relative_files: &[(String, u64)],
) -> Result<Vec<(PathBuf, String)>, String> {
    let mut found = Vec::new();
    for (relative, _size) in relative_files {
        let mut local = local_folder.to_path_buf();
        for part in relative.split('/') {
            if !is_plain_name(part) {
                return Err(fl!("files-unsafe-name", name = relative.as_str()));
            }
            local.push(part);
        }
        found.push((local, remotething_files::join(remote_folder, relative)));
    }
    Ok(found)
}

/// A single name, with no way up and no separator of this system in it.
pub(crate) fn is_plain_name(part: &str) -> bool {
    let mut components = Path::new(part).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(name)), None) if name == part
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_folder_becomes_one_upload_per_file_with_its_place_kept() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::create_dir(root.join("empty")).unwrap();
        std::fs::write(root.join("top.txt"), "1").unwrap();
        std::fs::write(root.join("a/b/deep.txt"), "2").unwrap();

        let found = uploads(root, "/srv/site").unwrap();

        assert_eq!(
            found,
            [
                (
                    root.join("a/b/deep.txt"),
                    "/srv/site/a/b/deep.txt".to_string()
                ),
                (root.join("top.txt"), "/srv/site/top.txt".to_string()),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn links_are_not_followed() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::write(root.join("real.txt"), "1").unwrap();
        std::os::unix::fs::symlink(root, root.join("loop")).unwrap();

        let found = uploads(root, "/srv").unwrap();

        assert_eq!(found.len(), 1);
    }

    #[test]
    fn a_missing_local_folder_is_an_error() {
        assert!(uploads(Path::new("/definitely/not/here"), "/srv").is_err());
    }

    #[test]
    fn a_remote_folder_becomes_one_download_per_file() {
        let files = vec![("a/b.txt".to_string(), 3), ("c.txt".to_string(), 4)];

        let found = downloads("/srv/site", Path::new("/tmp/site"), &files).unwrap();

        assert_eq!(
            found,
            [
                (
                    PathBuf::from("/tmp/site/a/b.txt"),
                    "/srv/site/a/b.txt".to_string()
                ),
                (
                    PathBuf::from("/tmp/site/c.txt"),
                    "/srv/site/c.txt".to_string()
                ),
            ]
        );
    }

    #[test]
    fn a_name_that_leads_out_of_the_folder_refuses_the_whole_download() {
        for bad in ["../x", "a/../../x", "/etc/passwd", "a//b", "./a", ""] {
            let files = vec![("fine.txt".to_string(), 1), (bad.to_string(), 1)];

            let refused = downloads("/srv", Path::new("/tmp/site"), &files);

            assert!(refused.is_err(), "{bad:?} was accepted");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_path_inside_a_name_is_refused() {
        let files = vec![("..\\x".to_string(), 1), ("C:\\x".to_string(), 1)];

        assert!(downloads("/srv", Path::new("C:\\site"), &files).is_err());
    }
}
