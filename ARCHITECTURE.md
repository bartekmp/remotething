# Architecture

Remotething is one native Rust binary that decodes and draws every protocol itself.

## Crates

Dependencies point one way, from the app down to `core`.

| Crate | Holds | Status |
| --- | --- | --- |
| `crates/core` | Connection model, folder inheritance, connections file | In place |
| `crates/secrets` | Secret type, OS keyring store with a timeout wrapper, in-memory store, encrypted-file store. Known hosts are planned | In place |
| `remote` | RDP, VNC and SSH sessions behind Cargo features | Planned |
| `files` | SFTP, SCP, FTP, SMB, NFS backends and the transfer queue | Planned |
| `crates/app` | iced UI: connection tree, tabs, properties editor, credentials and settings screens so far. Binary name `remotething` | In place |
| `xtask` | Lint, test and build commands for developers and CI | In place |

A crate is added when it has code to hold, not before.

## Rules

1. Protocol code knows nothing about the UI.
2. `core` needs no network and no window, so all of it is unit-tested.
3. No `unsafe` outside one small platform module; the workspace forbids it by default.
4. Each external protocol library sits behind our own narrow module so it can be swapped.

## Decisions

Short records live in `docs/adr/`. The M0 prototypes and their findings are in `spikes/` and `docs/spikes/m0-findings.md`.
