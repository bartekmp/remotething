# 2. IronRDP sub-crates, not the meta crate

Date: 2026-11-13. Status: accepted.

## Decision

Depend on `ironrdp-connector`, `ironrdp-session`, `ironrdp-graphics`, `ironrdp-pdu` and their siblings directly. Do not use the `ironrdp` meta crate.

## Why

The meta crate 0.17 lags its sub-crates and pins a pre-release of `curve25519-dalek` that cannot be resolved together with russh. The current sub-crates use final releases and share one lockfile with russh 0.65.

## Consequences

- Sub-crate versions are pinned and bumped together.
- All IronRDP use stays behind our own module, because the sub-crate APIs change between releases.
