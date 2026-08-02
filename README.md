# Attacca

**A native Roon client for Linux.** Roon ships desktop clients for Windows and macOS, but Linux users only get the headless Roon Server and Roon Bridge — the community has been asking for a GUI [since 2017](https://community.roonlabs.com/t/linux-roon-control-gui-please-not-on-roadmap-you-may-try-to-use-wine/23081). Attacca aims to be the daily-driver client Linux never got: browse your library and TIDAL/Qobuz, control every zone, and play to the machine it runs on.

*Attacca* (It., music): proceed to the next movement without pause.

> **Status: working alpha.** Qt 6/QML desktop app with Now Playing, full
> browse/search (library, TIDAL, Qobuz), live queue with click-to-jump,
> keyboard shortcuts, and an MPRIS2 bridge (media keys / desktop widgets).
> Daily-driven against Roon 2.70.

## Architecture

```mermaid
flowchart LR
    UI[attacca-ui\nQt 6 / QML — planned] --> CORE[attacca-core\nsession + state models]
    CLI[attacca-cli\nspike / debug tool] --> CORE
    CORE -- "SOOD (UDP 9003) + MOO over WebSocket\nofficial extension API, via roon-api crate" --> ROON[(Roon Core)]
    ROON -- RAAT --> BRIDGE[Roon Bridge\nlocal audio endpoint]
```

- **Control plane**: Roon's official, Apache-2.0-licensed extension API (the protocol behind [node-roon-api](https://github.com/RoonLabs/node-roon-api)), via the [`roon-api`](https://crates.io/crates/roon-api) Rust SDK. The Core is discovered via SOOD multicast; its SOOD response advertises the MOO/WebSocket port (`http_port`).
- **Audio plane**: a locally running [Roon Bridge](https://help.roonlabs.com/portal/kb/articles/linux-install) makes this machine a first-class RAAT zone. Attacca will offer a guided setup that downloads Bridge from Roon's servers (Roon's terms do not permit bundling it).
- **UI (planned)**: Qt 6 / QML — GPU scene graph, virtualized grids for large artwork libraries, first-class Wayland fractional scaling.

## What it can and cannot become

Built on the official API (see [research.md](research.md) for the fully sourced analysis):

| Works | Out of reach (API ceiling) |
|---|---|
| Transport, zones, grouping, volume | Queue editing (only "play from here") |
| Live queue + now-playing display | Playlist creation/editing |
| Library, TIDAL & Qobuz browsing, search | DSP configuration, signal path |
| Artwork, internet radio | Daily Mixes / Home recommendations, metadata editing |

Attacca is honest about being a very capable client, not a 1:1 clone of the native app.

## Try the spike

```sh
cargo run -p attacca-cli -- discover          # list Cores on the LAN
cargo run -p attacca-cli --                   # pair + watch zones (approve "Attacca"
                                              #   in Roon Settings → Extensions on first run)
cargo run -p attacca-cli -- toggle kitchen    # play/pause a zone by name substring
```

Pairing tokens are stored in `~/.config/attacca/tokens.json`.

## Install

Local (user scope):

```sh
cargo build --release -p attacca-ui
install -Dm755 target/release/attacca-ui ~/.local/bin/attacca-ui
install -Dm644 packaging/attacca.desktop ~/.local/share/applications/attacca.desktop
install -Dm644 packaging/attacca.svg ~/.local/share/icons/hicolor/scalable/apps/attacca.svg
```

Arch: `packaging/aur/PKGBUILD` (point `url` at the public remote first).
Flatpak: `packaging/flatpak/` (untested skeleton; cargo sources need vendoring).

## Development

For QML language-server support in your editor, create an untracked
`crates/attacca-ui/.qmlls.ini` pointing at your own checkout — the path must be
absolute, which is why it is not committed:

```ini
[General]
buildDir="/absolute/path/to/attacca/target/cxxqt/qml_modules"
no-cmake-calls=true
```

## Roadmap

1. ✅ Protocol spike: discover, pair, subscribe, control
2. ✅ Now Playing + zone picker + volume (QML)
3. ✅ Browse: artwork grid, search, play actions (TIDAL/Qobuz included)
4. ✅ Queue view, keyboard shortcuts, MPRIS2 bridge, icons/backdrop polish
5. Zone grouping UI
6. Guided Roon Bridge setup (waiting for Roon's .NET 10 Bridge, 2026-08-30;
   PipeWire coexistence via `plug:pipewire`)
7. Public repo, Flatpak/AUR publication, forum announcement
8. Upstream the queue protocol support to shin1ohno/roon-rs

## Legal

Attacca is an independent community project, not affiliated with, endorsed by, or supported by Roon Labs LLC or Harman International. "Roon" is a trademark of Roon Labs LLC. Attacca uses only Roon's publicly published extension API and does not redistribute any Roon software. A Roon subscription and a Roon Core on your network are required.

License: [MIT](LICENSE).
