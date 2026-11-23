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
| `cargo xtask prune` | Deletes old build outputs of our crates, keeping the newest; `bump` runs it too |
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

## Against a real Windows host

Some RDP tests need a Windows machine with Remote Desktop switched on. They are ignored unless the host is given, and the password stays in the environment, never in a file:

```sh
RT_RDP_HOST=192.168.0.35 RT_RDP_USER=test RT_RDP_PASSWORD=... \
  cargo test -p remotething-rdp --test windows_host -- --ignored
  # Files both ways: types into the host's Run dialog, pastes in Explorer, leaves C:\rtrecv behind.
  cargo test -p remotething-rdp --test windows_files -- --ignored --test-threads 1
```

Run them one at a time (`--test-threads=1`, or name one test): a second connection of the same user takes the session over from the first.

Before a release, run the soak test for an hour. It moves the pointer every 5 seconds, resizes the desktop every minute and fails if the session drops or memory grows by more than 150 MB:

```sh
RT_RDP_SOAK_MINUTES=60 RT_RDP_HOST=... RT_RDP_USER=... RT_RDP_PASSWORD=... \
  cargo test --release -p remotething-rdp --test windows_host -- --ignored --nocapture a_long_session
```

The soak test reconnects when the connection breaks, as the app does, and fails if that happens more than once per five minutes. Under a resize every few seconds it breaks about once per 30 resizes: Windows 11 skips one TLS record on its side (found by decrypting a dump of the stream offline: the failing record decrypted with the next sequence number), so this is not something the client can prevent.

A nightly run on Jenkins needs the three variables as credentials bound to the job.

## Rules

1. One obvious place for each thing: flat modules, no framework layers.
2. A file over about 400 lines or a function over about 60 gets split.
3. Every public item in a library crate has a doc comment; the build fails without one.
4. A feature lands with its tests and its doc page in the same change.
5. Dependencies must have a permissive licence that combines with the GPL-3.0 of this project; `deny.toml` is the list.
6. Colours, radii and sizes come from `crates/app/src/theme.rs`, never from a screen.

## Git

- Work on a short-lived `wp-<id>-<name>` branch and merge by pull request with squash.
- `main` is protected; Jenkins must be green before a merge.
- Jenkins is not reachable from the internet, so GitHub's webhook goes through a smee.io channel to a small relay on the Jenkins machine (a user service, `smee-jenkins`), which signs the event again with the secret set in Jenkins and posts it to `/github-webhook/`. Jenkins also scans every 5 minutes if the relay is down.
- Commit messages are one short line.

## SMB, FTP and SFTP against a real server

The tests that need a server are ignored until it is given in the environment. For SMB, any Samba works; this runs one as yourself on port 4450 (the socket path must stay short, so use a short directory such as `/tmp/rts`):

```sh
mkdir -p /tmp/rts/{share,priv,lock,cache,state,pid,ncalrpc,logs}
printf '[global]\n workgroup = WORKGROUP\n server role = standalone server\n smb ports = 4450\n interfaces = 127.0.0.1\n bind interfaces only = yes\n private dir = /tmp/rts/priv\n lock directory = /tmp/rts/lock\n state directory = /tmp/rts/state\n cache directory = /tmp/rts/cache\n pid directory = /tmp/rts/pid\n ncalrpc dir = /tmp/rts/ncalrpc\n passdb backend = tdbsam:/tmp/rts/priv/passdb.tdb\n map to guest = never\n load printers = no\n disable spoolss = yes\n\n[files]\n path = /tmp/rts/share\n read only = no\n valid users = %s\n force user = %s\n' "$USER" "$USER" > /tmp/rts/smb.conf
pdbedit -s /tmp/rts/smb.conf -a -t -u "$USER"   # reads the password twice from standard input
smbd -s /tmp/rts/smb.conf -l /tmp/rts/logs --foreground --no-process-group &
RT_SMB_HOST=127.0.0.1 RT_SMB_PORT=4450 RT_SMB_USER=$USER RT_SMB_PASSWORD=... RT_SMB_SHARE=files \
  cargo test -p remotething-files --test smb_server -- --ignored
```

Do not run it with `-i`: that serves one connection at a time and the tests open several. The FTP, SFTP and SCP tests are described at the top of `crates/files/tests/ftp_server.rs`, `sftp_server.rs` and `scp_server.rs`. For SCP, run `sshd` as yourself on a spare port with a config that has no `Subsystem sftp` line.
