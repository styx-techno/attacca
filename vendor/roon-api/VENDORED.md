# Vendored roon-api

Vendored from [shin1ohno/roon-rs](https://github.com/shin1ohno/roon-rs)
`crates/roon-api` at commit `db2697a32a832147202db98687052ae0ea0f6a03`
(v0.5.3, MIT OR Apache-2.0 — see LICENSE-MIT / LICENSE-APACHE).

Applied via `[patch.crates-io]` in the workspace root.

## Local changes

- `transport.rs`: added `subscribe_queue` / `unsubscribe_queue` /
  `play_from_here` (the `com.roonlabs.transport:2` queue surface upstream
  never implemented) plus the `QueueEvent` / `QueueItem` / `QueueChange`
  types in `queue.rs`.

Intent is to upstream these; drop the vendor dir once a release with queue
support exists on crates.io.
