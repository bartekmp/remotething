//! Build helper for the spikes: the same commands locally and in Jenkins.
//!
//! Usage: cargo xtask <lint | test | build linux | build windows>

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const LINUX_TARGET: &str = "x86_64-unknown-linux-gnu";
const WINDOWS_TARGET: &str = "x86_64-pc-windows-msvc";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let stage: &[&[&str]] = match args.as_slice() {
        ["lint"] => &[
            &["fmt", "--all", "--check"],
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ],
        ["test"] => &[&["test", "--workspace"]],
        ["build", "linux"] => &[&[
            "build",
            "--workspace",
            "--release",
            "--target",
            LINUX_TARGET,
        ]],
        // cargo-xwin supplies the MSVC SDK; it needs XWIN_ACCEPT_LICENSE=1.
        ["build", "windows"] => &[&[
            "xwin",
            "build",
            "--workspace",
            "--release",
            "--target",
            WINDOWS_TARGET,
        ]],
        _ => {
            eprintln!("usage: cargo xtask <lint | test | build linux | build windows>");
            return ExitCode::FAILURE;
        }
    };

    for cargo_args in stage {
        if !run_cargo(&spikes_root(), cargo_args) {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn spikes_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the spikes directory")
        .to_path_buf()
}

fn run_cargo(directory: &Path, args: &[&str]) -> bool {
    eprintln!("==> {}: cargo {}", directory.display(), args.join(" "));
    let status = Command::new("cargo")
        .args(args)
        .current_dir(directory)
        .status();
    match status {
        Ok(status) => status.success(),
        Err(error) => {
            eprintln!("could not start cargo: {error}");
            false
        }
    }
}
