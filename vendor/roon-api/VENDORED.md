# Vendored roon-api

Vendored from [shin1ohno/roon-rs](https://github.com/shin1ohno/roon-rs)
`crates/roon-api` at commit `db2697a32a832147202db98687052ae0ea0f6a03`
(v0.5.3, MIT OR Apache-2.0 — see LICENSE-MIT / LICENSE-APACHE).

Applied via `[patch.crates-io]` in the workspace root.

## Local changes

Queue support for `com.roonlabs.transport:2`, which upstream does not cover:

- `queue.rs`: `QueueItem`, `QueueChange`, `QueueOperation`, `QueueEvent`.
- `transport.rs`: `subscribe_queue`, `unsubscribe_queue`, `play_from_here`.
- `lib.rs`: the module and its re-exports, plus `LoopMode`, which upstream
  defines but does not re-export from the crate root.

`src/` is byte-identical to
[shin1ohno/roon-rs#13](https://github.com/shin1ohno/roon-rs/pull/13), so the
code Attacca runs is exactly the code under review. Keep it that way — if the
PR is revised, re-sync rather than patching here.

`Cargo.toml` is the one file that legitimately differs: the upstream crate is
part of a workspace and uses path dependencies and inherited lints, whereas
this copy has to stand alone. Those differences must not be sent upstream.

Drop this directory once a release with queue support reaches crates.io.
