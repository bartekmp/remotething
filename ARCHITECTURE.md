# Architecture

Remotething is one native Rust binary that decodes and draws every protocol itself.

## Crates

Dependencies point one way, from the app down to `core`.

| Crate | Holds | Status |
| --- | --- | --- |
| `crates/core` | Connection model, folder inheritance, connections file | In place |
| `crates/secrets` | Secret type, OS keyring store with a timeout wrapper, in-memory store, encrypted-file store, known SSH host keys | In place |
| `crates/remote` | Protocol sessions behind Cargo features. SSH shell so far; VNC is planned | In place |
| `crates/rdp` | RDP sessions over IronRDP: NLA sign-in, certificate pinning, frames out, input in. Used by the RDP tab | In place |
| `crates/vnc` | VNC sessions over vnc-rs: password sign-in over TCP or any byte stream, frames and pointer out, input in. Not used by the app yet | In place |
| `crates/files` | Remote file systems. SFTP listing, upload and download so far; the other backends and the transfer queue are planned | In place |
| `crates/app` | iced UI: connection tree, tabs with SSH terminals and a file pane, properties editor, credentials and settings screens so far. Binary name `remotething` | In place |
| `xtask` | Lint, test and build commands for developers and CI | In place |

A crate is added when it has code to hold, not before.

## Rules

1. Protocol code knows nothing about the UI.
2. `core` needs no network and no window, so all of it is unit-tested.
3. No `unsafe` outside one small platform module; the workspace forbids it by default.
4. Each external protocol library sits behind our own narrow module so it can be swapped.

## Decisions

Short records live in `docs/adr/`. The M0 prototypes and their findings are in `spikes/` and `docs/spikes/m0-findings.md`.
