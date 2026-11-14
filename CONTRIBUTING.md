# Contributing

## Setup

Install Rust through rustup; `rust-toolchain.toml` selects the version. Then:

```sh
cargo install --locked cargo-nextest cargo-deny cargo-deb cargo-generate-rpm
```

## Commands

The same commands run locally and in Jenkins.

| Command | Does |
| --- | --- |
| `cargo xtask lint` | `rustfmt`, `clippy -D warnings`, `cargo deny` |
| `cargo xtask test` | Unit tests through `cargo nextest` |
| `cargo xtask build linux` | Release build for Linux |
| `cargo xtask build windows` | Cross-build for Windows; needs `cargo-xwin` and `XWIN_ACCEPT_LICENSE=1` |
| `cargo xtask package linux` | `.deb` and `.rpm` in `dist/`, from the Linux build |
| `cargo xtask package windows` | Portable zip in `dist/`, from the Windows build |
| `mdbook build docs` | The user guide |

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
