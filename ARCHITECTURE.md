# Architecture

Remotething is one native Rust binary that decodes and draws every protocol itself.

## Crates

Dependencies point one way, from the app down to `core`.

| Crate | Holds |
| --- | --- |
| `crates/core` | Connection model, folder inheritance, connections file, PuTTY and WSL import |
| `crates/secrets` | Secret type, OS keyring store with a timeout wrapper, in-memory store, encrypted-file store, known host keys and pinned certificates |
| `crates/remote` | SSH sessions: shells, jump hosts, port forwards, SFTP and SCP channels |
| `crates/rdp` | RDP sessions over IronRDP: NLA sign-in, certificate pinning, frames out, input in, clipboard |
| `crates/vnc` | VNC sessions over vnc-rs: password and Tight sign-in, frames and pointer out, input in |
| `crates/files` | Remote file systems behind one interface: SFTP, SCP, FTP and FTPS, SMB, NFS |
| `crates/capture` | Writers for recordings and screenshots, with no UI code |
| `crates/app` | iced UI: connection tree, tabs, terminals, file panes, desktops, properties, credentials and settings. Binary name `remotething` |
| `xtask` | Lint, test, build, package, licence and version commands for developers and CI |

A crate is added when it has code to hold, not before.

## Rules

1. Protocol code knows nothing about the UI.
2. `core` needs no network and no window, so all of it is unit-tested.
3. No `unsafe` code: the workspace forbids it.
4. Each external protocol library sits behind our own narrow module so it can be swapped.

## Decisions

Short records live in `docs/adr/`. The M0 prototypes and their findings are in `spikes/` and `docs/spikes/m0-findings.md`.
