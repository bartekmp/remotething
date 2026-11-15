# Contributing

## Setup

Install Rust through rustup; `rust-toolchain.toml` selects the version. Then:

```sh
cargo install --locked cargo-nextest cargo-deny cargo-deb cargo-generate-rpm
```

## Commands

The same commands run locally and in Jenkins. Pull requests run lint, tests and the Linux build; the slower Windows build and package run on `main` and tags. Run `cargo xtask build windows` yourself when a change touches Windows-only code.

| Command | Does |
| --- | --- |
| `cargo xtask lint` | `rustfmt`, `clippy -D warnings`, `cargo deny` |
| `cargo xtask test` | Unit tests through `cargo nextest` |
| `cargo xtask build linux` | Release build for Linux |
| `cargo xtask build windows` | Cross-build for Windows; needs `cargo-xwin` and `XWIN_ACCEPT_LICENSE=1` |
| `cargo xtask package linux` | `.deb` and `.rpm` in `dist/`, from the Linux build |
| `cargo xtask package windows` | Portable zip in `dist/`, from the Windows build |
| `cargo xtask bump minor` or `patch` | Raises the version; see below |
| `mdbook build docs` | The user guide |

`Taskfile.yml` wraps these for everyday use: `task run`, `task test`, `task check` (lint, then tests). Run `task` for the list. It needs [Task](https://taskfile.dev).

## Versions

The version is `major.minor.patch` ([Semantic Versioning](https://semver.org)) and lives in one place: `[workspace.package]` in `Cargo.toml`. Every crate and the app's `--version` take it from there.

Every pull request that changes anything under `crates/` raises it once:

| The pull request is | Run | Example |
| --- | --- | --- |
| A new feature | `cargo xtask bump minor` | 0.2.3 becomes 0.3.0 |
| A fix | `cargo xtask bump patch` | 0.2.3 becomes 0.2.4 |
| A change that breaks users' files or habits | `cargo xtask bump major` | 0.2.3 becomes 1.0.0 |

1. Describe the change in one line under "Unreleased" in `CHANGELOG.md`, in words a user would understand.
2. Run the bump as the last commit of the pull request. It updates `Cargo.toml`, `Cargo.lock` and moves the changelog lines under the new version.
3. Jenkins fails a pull request whose crates changed but whose version did not.

Changes to docs, CI or the build helper alone need no bump. If `main` moved on and took your number, merge `main` and bump again.

## The logo

`assets/logo/remotething.svg` is the source. The PNGs, the favicon and the pixels embedded in the app are made from it with [resvg](https://github.com/linebender/resvg) (`cargo install resvg`):

```sh
resvg -w 256 -h 256 assets/logo/remotething.svg assets/logo/remotething-256.png
resvg -w 128 -h 128 assets/logo/remotething.svg /tmp/icon-128.png
magick /tmp/icon-128.png -depth 8 rgba:crates/app/assets/icon-128.rgba
resvg -w 32 -h 32 assets/logo/remotething.svg docs/theme/favicon.png
cp assets/logo/remotething.svg docs/theme/favicon.svg
cp assets/logo/remotething.svg docs/src/logo.svg
```

## Rules

1. One obvious place for each thing: flat modules, no framework layers.
2. A file over about 400 lines or a function over about 60 gets split.
3. Every public item in a library crate has a doc comment; the build fails without one.
4. A feature lands with its tests and its doc page in the same change.
5. Dependencies must have a permissive licence; `deny.toml` is the list.
6. Colours, radii and sizes come from `crates/app/src/theme.rs`, never from a screen.

## Git

- Work on a short-lived `wp-<id>-<name>` branch and merge by pull request with squash.
- `main` is protected; Jenkins must be green before a merge.
- Commit messages are one short line.
