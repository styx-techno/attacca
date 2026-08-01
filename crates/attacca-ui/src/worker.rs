//! Background worker: owns the Roon client on a tokio runtime and mirrors
//! state into the QObject via `CxxQtThread::queue`.

use crate::bridge::qobject::App;
use attacca_core::{
    Browse, BrowseOptions, BrowseResult, ControlAction, Core, LoadOptions, RoonEvent, SeekMode,
    VolumeMode, Zone, ZoneEvent,
};
use core::pin::Pin;
use cxx_qt::CxxQtThread;
use cxx_qt_lib::{QList, QString, QStringList};
use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedReceiver;

#[derive(Debug)]
pub enum Cmd {
    SelectZone(usize),
    PlayPause,
    Next,
    Previous,
    SetVolume(f64),
    Seek(f64),
    BrowseHome,
    BrowseInto(String),
    BrowseBack,
    Search(String),
    LoadMore,
}

const PAGE_SIZE: u32 = 100;

pub fn run(qt: CxxQtThread<App>, rx: UnboundedReceiver<Cmd>) {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            set_state(&qt, &format!("error: {e}"));
            return;
        }
    };
    if let Err(e) = rt.block_on(main_loop(&qt, rx)) {
        tracing::error!("worker exited: {e}");
        set_state(&qt, &format!("error: {e}"));
    }
}

fn push<F>(qt: &CxxQtThread<App>, f: F)
where
    F: FnOnce(Pin<&mut App>) + Send + 'static,
{
    let _ = qt.queue(f);
}

fn set_state(qt: &CxxQtThread<App>, state: &str) {
    let state = state.to_owned();
    push(qt, move |mut app| {
        app.as_mut().set_connection_state(QString::from(&state));
    });
}

/// One-shot SOOD scan for core addresses, used for direct artwork URLs
/// (`http://<core>:<port>/api/image/<key>`), which the paired client API does
/// not expose. Must complete BEFORE the client's own discovery starts: two
/// concurrent sockets on UDP 9003 with SO_REUSEPORT split unicast replies
/// between them, silently starving the client of discovery responses.
async fn prescan_cores() -> HashMap<String, (String, u16)> {
    let mut map = HashMap::new();
    let Ok((discovery, mut rx)) = roon_sood::SoodDiscovery::start().await else {
        tracing::warn!("SOOD prescan failed; artwork thumbnails disabled");
        return map;
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(core)) => {
                map.insert(core.core_id, (core.host.to_string(), core.http_port));
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            _ => break,
        }
    }
    discovery.stop().await;
    tracing::info!("SOOD prescan found {} core(s)", map.len());
    map
}

/// Discover via a short SOOD scan, then connect DIRECTLY to the core's
/// WebSocket. We never run the client's own discovery: Roon Bridge on the
/// same machine holds several UDP 9003 sockets, and with SO_REUSEPORT the
/// core's unicast replies land on only one 9003 socket — often not ours.
/// One short prescan is enough; the address is then used explicitly.
async fn main_loop(qt: &CxxQtThread<App>, mut rx: UnboundedReceiver<Cmd>) -> anyhow::Result<()> {
    let client = attacca_core::build_client()?;
    let mut events = client.events();

    loop {
        set_state(qt, "discovering");
        let Some((host, port)) = prescan_cores().await.into_values().next() else {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            continue;
        };

        // connect() registers and, on first run, waits for the user to
        // enable the extension in Roon — hence the "pairing" state.
        set_state(qt, "pairing");
        let core = match client.connect(&host, port).await {
            Ok(core) => core,
            Err(e) => {
                tracing::warn!("connect to {host}:{port} failed: {e}");
                set_state(qt, "retrying");
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
        };

        let name = core.display_name().to_owned();
        tracing::info!("paired with core \"{name}\" ({})", core.display_version());
        let base = format!("http://{host}:{port}/api/image/");
        push(qt, move |mut app| {
            app.as_mut().set_core_name(QString::from(&name));
            app.as_mut().set_image_base(QString::from(&base));
            app.as_mut().set_connection_state(QString::from("connected"));
        });

        // Runs until the core connection drops, then we rediscover.
        if let Err(e) = session(core, &mut events, &mut rx, qt).await {
            tracing::warn!("session ended: {e}");
        }
    }
}

/// Everything the UI shows about the selected zone.
fn push_view(qt: &CxxQtThread<App>, zone: Option<&Zone>) {
    let (title, artist, album) = zone
        .and_then(|z| z.now_playing.as_ref())
        .map(|np| match &np.three_line {
            Some(t) => (
                t.line1.clone(),
                t.line2.clone().unwrap_or_default(),
                t.line3.clone().unwrap_or_default(),
            ),
            None => (np.one_line.line1.clone(), String::new(), String::new()),
        })
        .unwrap_or_default();

    let play_state = zone.map(|z| format!("{:?}", z.state)).unwrap_or_default();
    let can_next = zone.is_some_and(|z| z.is_next_allowed);
    let can_previous = zone.is_some_and(|z| z.is_previous_allowed);
    let seek = zone.and_then(|z| z.seek_position).unwrap_or(0.0);
    let len = zone
        .and_then(|z| z.now_playing.as_ref())
        .and_then(|np| np.length)
        .unwrap_or(0.0);
    let vol = zone.and_then(|z| z.outputs.first()).and_then(|o| o.volume.as_ref());
    let (has_volume, volume, volume_min, volume_max) = match vol {
        Some(v) => (true, v.value, v.min, v.max),
        None => (false, 0.0, 0.0, 100.0),
    };

    push(qt, move |mut app| {
        app.as_mut().set_title(QString::from(&title));
        app.as_mut().set_artist(QString::from(&artist));
        app.as_mut().set_album(QString::from(&album));
        app.as_mut().set_play_state(QString::from(&play_state));
        app.as_mut().set_can_next(can_next);
        app.as_mut().set_can_previous(can_previous);
        app.as_mut().set_seek_position(seek);
        app.as_mut().set_track_length(len);
        app.as_mut().set_has_volume(has_volume);
        app.as_mut().set_volume(volume);
        app.as_mut().set_volume_min(volume_min);
        app.as_mut().set_volume_max(volume_max);
    });
}

fn push_zone_list(qt: &CxxQtThread<App>, zones: &[Zone], sel: usize) {
    let names: Vec<String> = zones.iter().map(|z| z.display_name.clone()).collect();
    let index = sel as i32;
    push(qt, move |mut app| {
        let mut list = QList::<QString>::default();
        for n in &names {
            list.append(QString::from(n));
        }
        app.as_mut().set_zone_list(QStringList::from(&list));
        app.as_mut().set_zone_index(index);
    });
}

/// Fetch album art via the Core's HTTP image service into the XDG cache and
/// point the UI at the file. `None` clears the artwork.
fn update_art(qt: &CxxQtThread<App>, core: &Core, key: Option<&str>, last: &mut Option<String>) {
    let key = key.map(str::to_owned);
    if *last == key {
        return;
    }
    *last = key.clone();

    let Some(key) = key else {
        push(qt, |mut app| app.as_mut().set_art_url(QString::default()));
        return;
    };

    let image = core.image();
    let qt = qt.clone();
    tokio::spawn(async move {
        let safe: String = key.chars().filter(char::is_ascii_alphanumeric).collect();
        let dir = dirs_next::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("attacca")
            .join("art");
        let path = dir.join(format!("{safe}.jpg"));

        if !path.exists() {
            let opts = attacca_core::ImageOptions {
                scale: Some("fit".into()),
                width: Some(1024),
                height: Some(1024),
                format: Some("image/jpeg".into()),
            };
            let Ok(bytes) = image.get_image(&key, &opts).await else {
                return;
            };
            if std::fs::create_dir_all(&dir).is_err() || std::fs::write(&path, bytes).is_err() {
                return;
            }
        }

        let url = format!("file://{}", path.display());
        push(&qt, move |mut app| {
            app.as_mut().set_art_url(QString::from(&url));
        });
    });
}

/// Server-side browse session state. One session, one hierarchy at a time:
/// "browse" for navigation, "search" for search results.
struct BrowseCtx {
    svc: Browse,
    hierarchy: String,
    count: u32,
    loaded: u32,
}

impl BrowseCtx {
    /// Apply a browse() result: new list → reset UI model and load page one;
    /// message → toast. Other actions (none/replace/remove) are ignored for now.
    async fn apply(&mut self, qt: &CxxQtThread<App>, result: BrowseResult) -> anyhow::Result<()> {
        match result.action.as_str() {
            "list" => {
                let Some(list) = result.list else {
                    return Ok(());
                };
                self.count = list.count;
                self.loaded = 0;
                tracing::info!(
                    "browse: list \"{}\" (level {}, {} items)",
                    list.title,
                    list.level,
                    list.count
                );
                let title = list.title.clone();
                let level = list.level as i32;
                let count = list.count as i32;
                let in_search = self.hierarchy == "search";
                push(qt, move |mut app| {
                    app.as_mut().browse_reset(QString::from(&title), level, count, in_search);
                });
                self.load_page(qt).await?;
            }
            "message" => {
                let msg = result.item.map(|i| i.title).unwrap_or_else(|| "Done".into());
                push(qt, move |mut app| {
                    app.as_mut().toast(QString::from(&msg));
                });
            }
            _ => {}
        }
        Ok(())
    }

    async fn load_page(&mut self, qt: &CxxQtThread<App>) -> anyhow::Result<()> {
        if self.loaded >= self.count {
            return Ok(());
        }
        let result = self
            .svc
            .load(LoadOptions {
                hierarchy: Some(self.hierarchy.clone()),
                offset: Some(self.loaded),
                count: Some(PAGE_SIZE),
                ..Default::default()
            })
            .await?;
        self.loaded += result.items.len() as u32;
        tracing::info!("browse: loaded {} item(s) ({}/{})", result.items.len(), self.loaded, self.count);
        if result.items.is_empty() {
            // Defensive: never loop forever on a misbehaving list.
            self.loaded = self.count;
            return Ok(());
        }

        let json = serde_json::Value::Array(
            result
                .items
                .iter()
                .map(|it| {
                    serde_json::json!({
                        "title": it.title,
                        "subtitle": it.subtitle.clone().unwrap_or_default(),
                        "imageKey": it.image_key.clone().unwrap_or_default(),
                        "itemKey": it.item_key.clone().unwrap_or_default(),
                        "hint": it.hint.clone().unwrap_or_default(),
                    })
                })
                .collect(),
        )
        .to_string();
        push(qt, move |mut app| {
            app.as_mut().browse_items(QString::from(&json));
        });
        Ok(())
    }

    /// browse() with busy indication and error tolerance; applies the result.
    async fn go(&mut self, qt: &CxxQtThread<App>, opts: BrowseOptions) {
        push(qt, |mut app| app.as_mut().set_browse_busy(true));
        match self.svc.browse(opts).await {
            Ok(result) => {
                if let Err(e) = self.apply(qt, result).await {
                    tracing::warn!("browse apply failed: {e}");
                }
            }
            Err(e) => tracing::warn!("browse failed: {e}"),
        }
        push(qt, |mut app| app.as_mut().set_browse_busy(false));
    }

    async fn home(&mut self, qt: &CxxQtThread<App>, zone_id: Option<String>) {
        self.hierarchy = "browse".to_owned();
        self.go(
            qt,
            BrowseOptions {
                hierarchy: Some("browse".into()),
                pop_all: Some(true),
                zone_or_output_id: zone_id,
                ..Default::default()
            },
        )
        .await;
    }
}

async fn session(
    core: Core,
    events: &mut tokio::sync::broadcast::Receiver<RoonEvent>,
    rx: &mut UnboundedReceiver<Cmd>,
    qt: &CxxQtThread<App>,
) -> anyhow::Result<()> {
    let transport = core.transport();
    let mut zone_rx = transport.subscribe_zones().await?;

    let mut zones: Vec<Zone> = Vec::new();
    let mut sel_id: Option<String> = None;
    let mut last_art: Option<String> = None;
    let mut browse = BrowseCtx {
        svc: core.browse(),
        hierarchy: "browse".to_owned(),
        count: 0,
        loaded: 0,
    };
    let mut did_home = false;

    loop {
        tokio::select! {
            ev = zone_rx.recv() => {
                let Some(ev) = ev else { return Ok(()) };
                let mut seek_only = false;
                match ev {
                    ZoneEvent::Initial(zs) => {
                        tracing::info!("received {} zone(s)", zs.len());
                        zones = zs;
                    }
                    ZoneEvent::Added(zs) | ZoneEvent::Changed(zs) => {
                        for z in zs {
                            match zones.iter_mut().find(|e| e.zone_id == z.zone_id) {
                                Some(e) => *e = z,
                                None => zones.push(z),
                            }
                        }
                    }
                    ZoneEvent::Removed(ids) => zones.retain(|z| !ids.contains(&z.zone_id)),
                    ZoneEvent::Seeked(seeks) => {
                        seek_only = true;
                        for s in &seeks {
                            if let Some(z) = zones.iter_mut().find(|z| z.zone_id == s.zone_id) {
                                z.seek_position = s.seek_position;
                            }
                        }
                    }
                }

                zones.sort_by(|a, b| a.display_name.cmp(&b.display_name));
                if sel_id.as_deref().map_or(true, |id| !zones.iter().any(|z| z.zone_id == id)) {
                    sel_id = zones.first().map(|z| z.zone_id.clone());
                }
                let sel = zones.iter().position(|z| Some(z.zone_id.as_str()) == sel_id.as_deref()).unwrap_or(0);
                let zone = zones.get(sel);

                if seek_only {
                    if let Some(pos) = zone.and_then(|z| z.seek_position) {
                        push(qt, move |mut app| app.as_mut().set_seek_position(pos));
                    }
                } else {
                    push_zone_list(qt, &zones, sel);
                    push_view(qt, zone);
                    let key = zone
                        .and_then(|z| z.now_playing.as_ref())
                        .and_then(|np| np.image_key.as_deref());
                    update_art(qt, &core, key, &mut last_art);
                }

                if !did_home {
                    did_home = true;
                    browse.home(qt, sel_id.clone()).await;
                }
            }

            cmd = rx.recv() => {
                let Some(cmd) = cmd else { return Ok(()) };
                let sel = zones.iter().position(|z| Some(z.zone_id.as_str()) == sel_id.as_deref());
                let zone = sel.and_then(|i| zones.get(i));

                let result = match cmd {
                    Cmd::SelectZone(i) => {
                        if let Some(z) = zones.get(i) {
                            sel_id = Some(z.zone_id.clone());
                            push_zone_list(qt, &zones, i);
                            push_view(qt, Some(z));
                            let key = z.now_playing.as_ref().and_then(|np| np.image_key.as_deref());
                            update_art(qt, &core, key, &mut last_art);
                        }
                        Ok(())
                    }
                    Cmd::PlayPause => match zone {
                        Some(z) => transport.control(&z.zone_id, ControlAction::PlayPause).await,
                        None => Ok(()),
                    },
                    Cmd::Next => match zone {
                        Some(z) => transport.control(&z.zone_id, ControlAction::Next).await,
                        None => Ok(()),
                    },
                    Cmd::Previous => match zone {
                        Some(z) => transport.control(&z.zone_id, ControlAction::Previous).await,
                        None => Ok(()),
                    },
                    Cmd::SetVolume(v) => match zone.and_then(|z| z.outputs.first()) {
                        Some(o) => transport.change_volume(&o.output_id, VolumeMode::Absolute, v).await,
                        None => Ok(()),
                    },
                    Cmd::Seek(s) => match zone {
                        Some(z) => transport.seek(&z.zone_id, SeekMode::Absolute, s as i64).await,
                        None => Ok(()),
                    },
                    Cmd::BrowseHome => {
                        browse.home(qt, sel_id.clone()).await;
                        Ok(())
                    }
                    Cmd::BrowseInto(item_key) => {
                        let opts = BrowseOptions {
                            hierarchy: Some(browse.hierarchy.clone()),
                            item_key: Some(item_key),
                            zone_or_output_id: sel_id.clone(),
                            ..Default::default()
                        };
                        browse.go(qt, opts).await;
                        Ok(())
                    }
                    Cmd::BrowseBack => {
                        let opts = BrowseOptions {
                            hierarchy: Some(browse.hierarchy.clone()),
                            pop_levels: Some(1),
                            zone_or_output_id: sel_id.clone(),
                            ..Default::default()
                        };
                        browse.go(qt, opts).await;
                        Ok(())
                    }
                    Cmd::Search(query) => {
                        browse.hierarchy = "search".to_owned();
                        let opts = BrowseOptions {
                            hierarchy: Some("search".into()),
                            input: Some(query),
                            pop_all: Some(true),
                            zone_or_output_id: sel_id.clone(),
                            ..Default::default()
                        };
                        browse.go(qt, opts).await;
                        Ok(())
                    }
                    Cmd::LoadMore => {
                        if let Err(e) = browse.load_page(qt).await {
                            tracing::warn!("load more failed: {e}");
                        }
                        Ok(())
                    }
                };
                if let Err(e) = result {
                    tracing::warn!("command failed: {e}");
                }
            }

            ev = events.recv() => {
                match ev? {
                    RoonEvent::CoreLost { .. } | RoonEvent::CoreUnpaired { .. } => return Ok(()),
                    _ => {}
                }
            }
        }
    }
}
