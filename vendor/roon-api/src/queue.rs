//! Play-queue types for `com.roonlabs.transport:2/subscribe_queue`.
//!
//! Local addition to the vendored crate — see VENDORED.md.

use serde::{Deserialize, Serialize};

use crate::zone::{LineInfo, ThreeLineInfo, TwoLineInfo};

/// One entry in a zone's play queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub queue_item_id: u64,
    /// Track length in seconds.
    #[serde(default)]
    pub length: Option<f64>,
    #[serde(default)]
    pub image_key: Option<String>,
    pub one_line: LineInfo,
    #[serde(default)]
    pub two_line: Option<TwoLineInfo>,
    #[serde(default)]
    pub three_line: Option<ThreeLineInfo>,
}

/// An incremental change to a subscribed queue.
#[derive(Debug, Clone, Deserialize)]
pub struct QueueChange {
    /// "insert" or "remove".
    pub operation: String,
    pub index: usize,
    /// Present for "remove".
    #[serde(default)]
    pub count: Option<usize>,
    /// Present for "insert".
    #[serde(default)]
    pub items: Option<Vec<QueueItem>>,
}

/// Events yielded by [`crate::Transport::subscribe_queue`].
#[derive(Debug, Clone)]
pub enum QueueEvent {
    /// Initial full queue snapshot.
    Subscribed(Vec<QueueItem>),
    /// Incremental changes; apply in order.
    Changed(Vec<QueueChange>),
}

pub(crate) fn parse_queue_event(name: &str, body: &serde_json::Value) -> Option<QueueEvent> {
    match name {
        "Subscribed" => serde_json::from_value(body.get("items")?.clone())
            .ok()
            .map(QueueEvent::Subscribed),
        "Changed" => serde_json::from_value(body.get("changes")?.clone())
            .ok()
            .map(QueueEvent::Changed),
        _ => {
            tracing::debug!("unhandled queue event: {name}");
            None
        }
    }
}
