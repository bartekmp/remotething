//! Build helper: the same commands locally and in Jenkins.

mod prune;
mod version;

use std::path::PathBuf;
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: cargo xtask <lint | test | build linux | build windows | package linux | package flatpak | package windows | bump major|minor|patch | prune | version-check <git ref>>";

const LINUX_TARGET: &str = "x86_64-unknown-linux-gnu";
const WINDOWS_TARGET: &str = "x86_64-pc-windows-msvc";
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let succeeded = match args.as_slice() {
        ["lint"] => lint(),
        ["test"] => cargo(&["nextest", "run", "--workspace"]),
        ["build", "linux"] => cargo(&[
            "build",
            "--workspace",
            "--release",
            "--target",
            LINUX_TARGET,
        ]),
        ["build", "windows"] => build_windows(),
        ["package", "linux"] => package_linux(),
        ["package", "flatpak"] => package_flatpak(),
        ["package", "windows"] => package_windows(),
        ["bump", part] => bump(part),
        ["prune"] => prune_builds(),
        ["version-check", base] => version_check(base),
        _ => {
            eprintln!("{USAGE}");
            false
        }
    };

    if succeeded {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Deletes old build outputs of the workspace's crates, keeping the newest of each.
fn prune_builds() -> bool {
    let freed = prune::prune(&workspace_root().join("target"));
    println!("removed up to {} MB of old builds", freed / 1_000_000);
    true
}

fn lint() -> bool {
    cargo(&["fmt", "--all", "--check"])
        && cargo(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ])
        && cargo(&["deny", "check"])
}

/// cargo-xwin supplies the MSVC SDK; it needs XWIN_ACCEPT_LICENSE=1.
/// The C runtime is linked in, so the `.exe` runs without the Visual C++ Redistributable.
fn build_windows() -> bool {
    let args = [
        "xwin",
        "build",
        "--workspace",
        "--release",
        "--target",
        WINDOWS_TARGET,
    ];
    let mut command = Command::new("cargo");
    command
        .args(args)
        .env("RUSTFLAGS", "-C target-feature=+crt-static")
        .current_dir(workspace_root());
    run(command, &format!("cargo {}", args.join(" ")))
}

/// Wraps the Linux release build as `.deb` and `.rpm` in `dist/`. Run `build linux` first.
fn package_linux() -> bool {
    let Some(dist) = dist_directory() else {
        return false;
    };
    let dist = dist.to_string_lossy();

    cargo(&[
        "deb",
        "-p",
        "remotething",
        "--no-build",
        "--target",
        LINUX_TARGET,
        "--output",
        &dist,
    ]) && cargo(&[
        "generate-rpm",
        "-p",
        "crates/app",
        "--target",
        LINUX_TARGET,
        "--output",
        &dist,
    ])
}

/// Builds the Flatpak in its own SDK and writes a bundle to `dist/`. Needs `flatpak-builder`
/// and the freedesktop SDK with its Rust extension; it does not use `build linux`.
fn package_flatpak() -> bool {
    let Some(dist) = dist_directory() else {
        return false;
    };
    let manifest =
        workspace_root().join("crates/app/packaging/flatpak/io.github.bartekmp.Remotething.yml");
    let build_directory = workspace_root().join("target").join("flatpak-build");
    let repo = dist.join("flatpak-repo");
    let bundle = dist.join(format!("remotething-{VERSION}.flatpak"));
    let run = |program: &str, args: &[String]| {
        println!("==> {program} {}", args.join(" "));
        Command::new(program)
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
    };
    let text = |path: &std::path::Path| path.to_string_lossy().into_owned();

    // The builder is a package on some systems and a Flatpak itself on others.
    let installed = Command::new("flatpak-builder")
        .arg("--version")
        .output()
        .is_ok();
    let (program, mut build) = if installed {
        ("flatpak-builder", Vec::new())
    } else {
        (
            "flatpak",
            vec!["run".to_string(), "org.flatpak.Builder".to_string()],
        )
    };
    build.extend([
        "--force-clean".to_string(),
        "--disable-rofiles-fuse".to_string(),
        "--user".to_string(),
        "--install-deps-from=flathub".to_string(),
        format!("--repo={}", text(&repo)),
        text(&build_directory),
        text(&manifest),
    ]);

    run(program, &build)
        && run(
            "flatpak",
            &[
                "build-bundle".to_string(),
                text(&repo),
                text(&bundle),
                "io.github.bartekmp.Remotething".to_string(),
            ],
        )
}

/// Zips the Windows release build into `dist/`. Run `build windows` first.
fn package_windows() -> bool {
    let Some(dist) = dist_directory() else {
        return false;
    };
    let root = workspace_root();
    let archive = dist.join(format!("remotething-{VERSION}-windows-x86_64.zip"));
    let executable = target_directory()
        .join(WINDOWS_TARGET)
        .join("release")
        .join("remotething.exe");

    // `zip` adds to an existing archive, so start from nothing.
    let _ = std::fs::remove_file(&archive);
    let files = [
        executable,
        root.join("LICENSE"),
        root.join("NOTICE"),
        root.join("README.md"),
    ];

    let mut zip = Command::new("zip");
    // -j: store the files without their directories.
    zip.arg("-j").arg(&archive).args(&files);
    run(zip, "zip")
}

/// Raises the version in the manifest and the lockfile, and moves the
/// changelog's unreleased notes under the new version.
fn bump(part: &str) -> bool {
    let Some(part) = version::Part::named(part) else {
        eprintln!("{USAGE}");
        return false;
    };
    let root = workspace_root();
    let manifest_file = root.join("Cargo.toml");
    let changelog_file = root.join("CHANGELOG.md");

    let updated = std::fs::read_to_string(&manifest_file)
        .ok()
        .and_then(|manifest| {
            let next = version::in_manifest(&manifest)?.raised(part);
            Some((version::with_version(&manifest, next)?, next))
        });
    let Some((manifest, next)) = updated else {
        eprintln!(
            "no version under [workspace.package] in {}",
            manifest_file.display()
        );
        return false;
    };
    let changelog = std::fs::read_to_string(&changelog_file)
        .ok()
        .and_then(|changelog| version::with_release(&changelog, next, &version::today()));
    let Some(changelog) = changelog else {
        eprintln!(
            "no \"## Unreleased\" heading in {}",
            changelog_file.display()
        );
        return false;
    };

    if let Err(error) = std::fs::write(&manifest_file, manifest)
        .and_then(|()| std::fs::write(&changelog_file, changelog))
    {
        eprintln!("cannot write the new version: {error}");
        return false;
    }
    println!("version {next}");
    // Each release builds new binaries; the old ones are no use any more.
    prune_builds();
    // The lockfile repeats the version of every crate in the workspace.
    cargo(&["update", "--workspace", "--quiet"])
}

/// Fails when the crates differ from `base` but the version is not above the one there.
fn version_check(base: &str) -> bool {
    let root = workspace_root();
    let ours = std::fs::read_to_string(root.join("Cargo.toml"))
        .ok()
        .and_then(|manifest| version::in_manifest(&manifest));
    let theirs = git_output(&["show", &format!("{base}:Cargo.toml")])
        .and_then(|manifest| version::in_manifest(&manifest));
    let changed = git_output(&[
        "diff",
        "--name-only",
        &format!("{base}...HEAD"),
        "--",
        "crates",
    ]);
    let (Some(ours), Some(theirs), Some(changed)) = (ours, theirs, changed) else {
        eprintln!("cannot compare the version with {base}; is that ref fetched?");
        return false;
    };

    match version::check(ours, theirs, !changed.trim().is_empty()) {
        Ok(()) => {
            println!("version {ours} (target branch: {theirs})");
            true
        }
        Err(reason) => {
            eprintln!("{reason}");
            false
        }
    }
}

/// What a git command prints, or `None` when it fails.
fn git_output(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(workspace_root())
        .args(arguments)
        .output()
        .ok()?;
    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr).trim());
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// The workspace, found when xtask runs. The path cargo saw when it built xtask is not used: CI
/// shares one build cache between workspaces, so the binary may come from one that is gone.
fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    manifest_dir
        .parent()
        .expect("xtask lives inside the workspace")
        .to_path_buf()
}

/// Where cargo puts build output: `CARGO_TARGET_DIR`, or `target/` in the workspace.
fn target_directory() -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => workspace_root().join("target"),
    }
}

/// `dist/` in the workspace, created if needed.
fn dist_directory() -> Option<PathBuf> {
    let dist = workspace_root().join("dist");
    match std::fs::create_dir_all(&dist) {
        Ok(()) => Some(dist),
        Err(error) => {
            eprintln!("cannot create {}: {error}", dist.display());
            None
        }
    }
}

fn cargo(args: &[&str]) -> bool {
    let mut command = Command::new("cargo");
    command.args(args).current_dir(workspace_root());
    run(command, &format!("cargo {}", args.join(" ")))
}

fn run(mut command: Command, description: &str) -> bool {
    eprintln!("==> {description}");
    match command.status() {
        Ok(status) => status.success(),
        Err(error) => {
            eprintln!("could not start `{description}`: {error}");
            false
        }
    }
}
