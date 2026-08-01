//! attacca-core: the session layer between the UI and the Roon Core.
//!
//! For now this is a thin wrapper around the `roon-api` SDK (SOOD discovery +
//! MOO RPC over WebSocket). As the UI grows, zone/browse/queue state models
//! live here so the QML layer stays declarative.

use std::path::PathBuf;

pub use roon_api::{
    Browse, BrowseItem, BrowseList, BrowseOptions, BrowseResult, ControlAction, Core,
    FileTokenStore, ImageOptions, ImageService, LoadOptions, LoadResult, MuteAction, NowPlaying,
    Output, PairingState, PlayState, QueueChange, QueueEvent, QueueItem, RoonClient,
    RoonClientBuilder, RoonEvent, SeekMode, Transport, Volume, VolumeMode, Zone, ZoneEvent,
    ZoneSeek, ZoneSettings,
};

pub const EXTENSION_ID: &str = "org.attacca.client";
pub const DISPLAY_NAME: &str = "Attacca";
pub const PUBLISHER: &str = "Attacca contributors";
pub const EMAIL: &str = "max.uckrow@gmx.de";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Pairing tokens live in `~/.config/attacca/tokens.json`.
pub fn token_store_path() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("attacca")
        .join("tokens.json")
}

/// A `RoonClient` announcing the Attacca identity, with transport enabled and
/// tokens persisted, ready for `start_discovery()` or `connect(host, port)`.
pub fn build_client() -> anyhow::Result<RoonClient> {
    let client = RoonClientBuilder::new(EXTENSION_ID, DISPLAY_NAME, VERSION, PUBLISHER, EMAIL)
        .token_store(FileTokenStore::new(token_store_path()))
        .require_transport()
        .require_browse()
        .build()?;
    Ok(client)
}
