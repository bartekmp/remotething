//! Deletes old build outputs of the workspace's own crates, keeping the newest of each.
//!
//! Cargo never removes what an earlier build left behind, and every rebuild of a test
//! binary adds one more. Dependencies are left alone: they rarely change, and removing
//! one that another build still needs only makes it compile again.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Crates whose outputs are pruned: the workspace's own.
fn is_ours(name: &str) -> bool {
    let name = name.strip_prefix("lib").unwrap_or(name);
    name.starts_with("remotething") || name == "xtask"
}

/// Splits `libremotething_core-0123abcd4567ef89.rlib` into (`libremotething_core`, `rlib`).
/// Returns `None` for names without a 16-digit hexadecimal hash after the last `-`.
fn name_and_extension(file: &str) -> Option<(&str, &str)> {
    let (stem, extension) = file.rsplit_once('.').unwrap_or((file, ""));
    let (name, hash) = stem.rsplit_once('-')?;
    let is_hash = hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit());
    is_hash.then_some((name, extension))
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn size_of(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| size_of(&entry.path()))
        .sum()
}

/// Removes every path but the newest of each group. Returns the bytes freed.
fn remove_all_but_newest(groups: HashMap<(String, String), Vec<PathBuf>>) -> u64 {
    let mut freed = 0;
    for mut paths in groups.into_values() {
        paths.sort_by_key(|path| modified(path));
        paths.pop();
        for path in paths {
            let bytes = size_of(&path);
            let removed = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            if removed.is_ok() {
                freed += bytes;
            }
        }
    }
    freed
}

/// Old files of our crates in a `deps` folder.
fn prune_deps(deps: &Path) -> u64 {
    let mut groups: HashMap<(String, String), Vec<PathBuf>> = HashMap::new();
    for entry in fs::read_dir(deps).into_iter().flatten().flatten() {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if let Some((name, extension)) = name_and_extension(&file_name)
            && is_ours(name)
        {
            groups
                .entry((name.to_string(), extension.to_string()))
                .or_default()
                .push(entry.path());
        }
    }
    remove_all_but_newest(groups)
}

/// Old compilation sessions of our crates in an `incremental` folder.
fn prune_incremental(incremental: &Path) -> u64 {
    let mut crates: HashMap<(String, String), Vec<PathBuf>> = HashMap::new();
    for entry in fs::read_dir(incremental).into_iter().flatten().flatten() {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        // Folders are named `crate_name-<hash>`; the hash is shorter than in `deps`.
        let Some((name, _)) = file_name.rsplit_once('-') else {
            continue;
        };
        if is_ours(name) {
            crates
                .entry((name.to_string(), String::new()))
                .or_default()
                .push(entry.path());
        }
    }
    let mut freed = 0;
    for paths in crates.values() {
        // Sessions of the newest folder of each crate: keep the newest one too.
        let newest = paths.iter().max_by_key(|path| modified(path));
        if let Some(newest) = newest {
            let mut sessions: HashMap<(String, String), Vec<PathBuf>> = HashMap::new();
            for session in fs::read_dir(newest).into_iter().flatten().flatten() {
                sessions
                    .entry((String::new(), String::new()))
                    .or_default()
                    .push(session.path());
            }
            freed += remove_all_but_newest(sessions);
        }
    }
    freed + remove_all_but_newest(crates)
}

/// Prunes every `deps` and `incremental` folder under `target`, for any profile or target.
pub fn prune(target: &Path) -> u64 {
    let mut freed = 0;
    let mut folders = vec![target.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.is_dir() || path.is_symlink() {
                continue;
            }
            match entry.file_name().to_str() {
                Some("deps") => freed += prune_deps(&path),
                Some("incremental") => freed += prune_incremental(&path),
                Some("build" | "examples") => {}
                _ => folders.push(path),
            }
        }
    }
    freed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn touch(path: &Path, age_seconds: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x").unwrap();
        let when = SystemTime::now() - Duration::from_secs(age_seconds);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }

    #[test]
    fn file_names_split_into_name_and_extension() {
        assert_eq!(
            name_and_extension("libremotething_core-0123456789abcdef.rlib"),
            Some(("libremotething_core", "rlib"))
        );
        assert_eq!(
            name_and_extension("remotething-0123456789abcdef"),
            Some(("remotething", ""))
        );
        assert_eq!(name_and_extension("remotething.d"), None);
        assert_eq!(name_and_extension("serde-short.rlib"), None);
    }

    #[test]
    fn only_the_newest_build_of_our_crates_is_kept() {
        let directory = tempfile::tempdir().unwrap();
        let deps = directory.path().join("debug").join("deps");
        touch(&deps.join("remotething-aaaaaaaaaaaaaaaa"), 300);
        touch(&deps.join("remotething-bbbbbbbbbbbbbbbb"), 100);
        touch(&deps.join("remotething-cccccccccccccccc"), 200);
        touch(&deps.join("serde-aaaaaaaaaaaaaaaa.rlib"), 300);
        touch(&deps.join("serde-bbbbbbbbbbbbbbbb.rlib"), 100);

        let freed = prune(directory.path());

        assert_eq!(freed, 2);
        assert!(deps.join("remotething-bbbbbbbbbbbbbbbb").exists());
        assert!(!deps.join("remotething-aaaaaaaaaaaaaaaa").exists());
        assert!(!deps.join("remotething-cccccccccccccccc").exists());
        assert!(deps.join("serde-aaaaaaaaaaaaaaaa.rlib").exists());
    }

    #[test]
    fn old_incremental_sessions_of_our_crates_are_removed() {
        let directory = tempfile::tempdir().unwrap();
        let crate_dir = directory
            .path()
            .join("debug")
            .join("incremental")
            .join("remotething-1abc");
        touch(&crate_dir.join("s-old-aaa").join("data"), 300);
        touch(&crate_dir.join("s-new-bbb").join("data"), 10);
        for session in ["s-old-aaa", "s-new-bbb"] {
            let when = if session == "s-old-aaa" { 300 } else { 10 };
            let path = crate_dir.join(session);
            fs::File::open(&path)
                .unwrap()
                .set_modified(SystemTime::now() - Duration::from_secs(when))
                .unwrap();
        }

        prune(directory.path());

        assert!(crate_dir.join("s-new-bbb").exists());
        assert!(!crate_dir.join("s-old-aaa").exists());
    }
}
