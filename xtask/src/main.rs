//! Build helper: the same commands locally and in Jenkins.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: cargo xtask <lint | test | build linux | build windows | package linux | package windows>";

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
        ["package", "windows"] => package_windows(),
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
fn build_windows() -> bool {
    cargo(&[
        "xwin",
        "build",
        "--workspace",
        "--release",
        "--target",
        WINDOWS_TARGET,
    ])
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
    let files = [executable, root.join("LICENSE"), root.join("README.md")];

    let mut zip = Command::new("zip");
    // -j: store the files without their directories.
    zip.arg("-j").arg(&archive).args(&files);
    run(zip, "zip")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
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
