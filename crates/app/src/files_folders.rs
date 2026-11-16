//! Turning a folder into the list of file transfers it stands for.

use std::io;
use std::path::{Path, PathBuf};

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
pub(crate) fn downloads(
    remote_folder: &str,
    local_folder: &Path,
    relative_files: &[(String, u64)],
) -> Vec<(PathBuf, String)> {
    relative_files
        .iter()
        .map(|(relative, _size)| {
            let local = relative
                .split('/')
                .fold(local_folder.to_path_buf(), |path, part| path.join(part));
            (local, remotething_files::join(remote_folder, relative))
        })
        .collect()
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

        let found = downloads("/srv/site", Path::new("/tmp/site"), &files);

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
}
