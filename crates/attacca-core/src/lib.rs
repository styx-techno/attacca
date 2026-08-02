//! attacca-core: the session layer between the UI and the Roon Core.
//!
//! For now this is a thin wrapper around the `roon-api` SDK (SOOD discovery +
//! MOO RPC over WebSocket). As the UI grows, zone/browse/queue state models
//! live here so the QML layer stays declarative.

use std::path::PathBuf;

pub use roon_api::{
    Browse, BrowseItem, BrowseList, BrowseOptions, BrowseResult, ControlAction, Core,
    FileTokenStore, ImageOptions, ImageService, LoadOptions, LoadResult, MuteAction, NowPlaying,
    LoopMode, Output, PairingState, PlayState, QueueChange, QueueEvent, QueueItem, QueueOperation,
    RoonClient, RoonClientBuilder, RoonEvent, SeekMode, Transport, Volume, VolumeMode, Zone,
    ZoneEvent, ZoneSeek, ZoneSettings,
};

pub const EXTENSION_ID: &str = "org.attacca.client";
pub const DISPLAY_NAME: &str = "Attacca";
pub const PUBLISHER: &str = "Attacca contributors";
pub const EMAIL: &str = "5517989+styx-techno@users.noreply.github.com";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Pairing tokens live in `~/.config/attacca/<file>`.
pub fn token_store_path() -> PathBuf {
    config_file("tokens.json")
}

/// The CLI authorizes as its own extension so debug sessions never fight the
/// running app over one Core-side connection (same-identity connections get
/// reset by the Core).
pub fn cli_token_store_path() -> PathBuf {
    config_file("tokens-cli.json")
}

fn config_file(name: &str) -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("attacca")
        .join(name)
}

/// A `RoonClient` announcing the Attacca app identity, with transport+browse
/// enabled and tokens persisted, ready for discovery or `connect(host, port)`.
pub fn build_client() -> anyhow::Result<RoonClient> {
    build_named(EXTENSION_ID, DISPLAY_NAME, token_store_path())
}

/// Client for the debug CLI under its own extension identity.
pub fn build_cli_client() -> anyhow::Result<RoonClient> {
    build_named("org.attacca.cli", "Attacca CLI", cli_token_store_path())
}

fn build_named(id: &str, name: &str, tokens: PathBuf) -> anyhow::Result<RoonClient> {
    let client = RoonClientBuilder::new(id, name, VERSION, PUBLISHER, EMAIL)
        .token_store(FileTokenStore::new(tokens))
        .require_transport()
        .require_browse()
        .build()?;
    Ok(client)
}
