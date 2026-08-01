//! MPRIS2 bridge: exposes the selected zone as `org.mpris.MediaPlayer2.attacca`
//! so media keys and desktop widgets (KDE media controls, playerctl, …)
//! control Roon like any local player.

use crate::worker::Cmd;
use mpris_server::zbus::{self, fdo};
use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, Property, RootInterface,
    Server, Time, TrackId, Volume,
};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

/// Mirror of the selected zone's state, owned by the MPRIS player object and
/// refreshed by the worker on every zone update.
#[derive(Default, Clone, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_url: String,
    /// "Playing" | "Paused" | "Loading" | "Stopped" (Roon's PlayState debug names)
    pub play_state: String,
    pub length_us: i64,
    pub position_us: i64,
    pub can_next: bool,
    pub can_previous: bool,
    pub has_volume: bool,
    pub vol_min: f64,
    pub vol_max: f64,
    pub vol_value: f64,
}

struct Inner {
    now: NowPlaying,
    track_serial: u64,
}

pub struct Player {
    inner: Mutex<Inner>,
    cmds: UnboundedSender<Cmd>,
}

pub type MprisServer = Arc<Server<Player>>;

pub const BUS_NAME: &str = "org.mpris.MediaPlayer2.attacca";
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";

/// True when another instance already owns our MPRIS name; best-effort asks it
/// to raise its window. The caller should then exit rather than start a second
/// instance, which would fight the first over the Roon extension identity (the
/// Core resets connections when one identity connects twice).
///
/// The ownership check must come first: mpris-server requests the name with
/// replacement allowed, so simply registering would silently steal it from the
/// running instance and leave the name unowned once we exit.
pub async fn instance_already_running() -> bool {
    let Ok(conn) = zbus::Connection::session().await else {
        return false;
    };
    let Ok(dbus) = zbus::fdo::DBusProxy::new(&conn).await else {
        return false;
    };
    let Ok(name) = BUS_NAME.try_into() else {
        return false;
    };
    if !dbus.name_has_owner(name).await.unwrap_or(false) {
        return false;
    }
    if let Ok(proxy) =
        zbus::Proxy::new(&conn, BUS_NAME, OBJECT_PATH, "org.mpris.MediaPlayer2").await
    {
        let _ = proxy.call_method("Raise", &()).await;
    }
    true
}

/// Register on the session bus. Returns None (with a log line) when D-Bus is
/// unavailable — the app works fine without MPRIS.
pub async fn serve(cmds: UnboundedSender<Cmd>) -> Option<MprisServer> {
    let player = Player {
        inner: Mutex::new(Inner {
            now: NowPlaying::default(),
            track_serial: 0,
        }),
        cmds,
    };
    match Server::new("attacca", player).await {
        Ok(server) => Some(Arc::new(server)),
        Err(e) => {
            tracing::warn!("MPRIS unavailable: {e}");
            None
        }
    }
}

/// Replace the mirrored state and emit the resulting property changes.
pub fn update(server: &Option<MprisServer>, now: NowPlaying) {
    let Some(server) = server else { return };
    let props = server.imp().replace(now);
    if props.is_empty() {
        return;
    }
    let server = server.clone();
    tokio::spawn(async move {
        if let Err(e) = server.properties_changed(props).await {
            tracing::debug!("MPRIS properties_changed failed: {e}");
        }
    });
}

/// Position-only refresh: MPRIS clients poll Position, no signal needed.
pub fn update_position(server: &Option<MprisServer>, position_us: i64) {
    if let Some(server) = server {
        server.imp().inner.lock().unwrap().now.position_us = position_us;
    }
}

/// Late artwork arrival (worker caches art to disk asynchronously).
pub fn update_art(server: &Option<MprisServer>, art_url: String) {
    let Some(server) = server else { return };
    let metadata = {
        let mut inner = server.imp().inner.lock().unwrap();
        if inner.now.art_url == art_url {
            return;
        }
        inner.now.art_url = art_url;
        build_metadata(&inner)
    };
    let server = server.clone();
    tokio::spawn(async move {
        let _ = server.properties_changed([Property::Metadata(metadata)]).await;
    });
}

fn build_metadata(inner: &Inner) -> Metadata {
    let now = &inner.now;
    let mut b = Metadata::builder()
        .trackid(
            TrackId::try_from(format!("/org/attacca/track/{}", inner.track_serial))
                .unwrap_or(TrackId::NO_TRACK),
        );
    if !now.title.is_empty() {
        b = b.title(now.title.clone());
    }
    if !now.artist.is_empty() {
        b = b.artist([now.artist.clone()]);
    }
    if !now.album.is_empty() {
        b = b.album(now.album.clone());
    }
    if now.length_us > 0 {
        b = b.length(Time::from_micros(now.length_us));
    }
    if !now.art_url.is_empty() {
        b = b.art_url(now.art_url.clone());
    }
    b.build()
}

impl Player {
    fn replace(&self, mut now: NowPlaying) -> Vec<Property> {
        let mut inner = self.inner.lock().unwrap();
        let old = inner.now.clone();
        // The async art fetch may land before or after the zone update; keep
        // the existing art when the incoming state carries none.
        if now.art_url.is_empty() && now.title == old.title {
            now.art_url = old.art_url.clone();
        }
        if (&now.title, &now.artist, &now.album) != (&old.title, &old.artist, &old.album) {
            inner.track_serial += 1;
        }
        inner.now = now;

        let mut props = Vec::new();
        let now = inner.now.clone();
        if now.play_state != old.play_state {
            props.push(Property::PlaybackStatus(playback_status(&now.play_state)));
        }
        if (&now.title, &now.artist, &now.album, &now.art_url, now.length_us)
            != (&old.title, &old.artist, &old.album, &old.art_url, old.length_us)
        {
            props.push(Property::Metadata(build_metadata(&inner)));
        }
        if now.can_next != old.can_next {
            props.push(Property::CanGoNext(now.can_next));
        }
        if now.can_previous != old.can_previous {
            props.push(Property::CanGoPrevious(now.can_previous));
        }
        if volume_frac(&now) != volume_frac(&old) {
            props.push(Property::Volume(volume_frac(&now)));
        }
        props
    }

    fn send(&self, cmd: Cmd) {
        let _ = self.cmds.send(cmd);
    }

    fn now(&self) -> NowPlaying {
        self.inner.lock().unwrap().now.clone()
    }
}

fn playback_status(state: &str) -> PlaybackStatus {
    match state {
        "Playing" => PlaybackStatus::Playing,
        "Paused" | "Loading" => PlaybackStatus::Paused,
        _ => PlaybackStatus::Stopped,
    }
}

fn volume_frac(now: &NowPlaying) -> f64 {
    if !now.has_volume || now.vol_max <= now.vol_min {
        return 1.0;
    }
    ((now.vol_value - now.vol_min) / (now.vol_max - now.vol_min)).clamp(0.0, 1.0)
}

impl RootInterface for Player {
    async fn raise(&self) -> fdo::Result<()> {
        self.send(Cmd::RaiseWindow);
        Ok(())
    }
    async fn quit(&self) -> fdo::Result<()> {
        Ok(())
    }
    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Ok(())
    }
    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(true)
    }
    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn identity(&self) -> fdo::Result<String> {
        Ok("Attacca".into())
    }
    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("attacca".into())
    }
    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }
    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }
}

impl PlayerInterface for Player {
    async fn next(&self) -> fdo::Result<()> {
        self.send(Cmd::Next);
        Ok(())
    }
    async fn previous(&self) -> fdo::Result<()> {
        self.send(Cmd::Previous);
        Ok(())
    }
    async fn pause(&self) -> fdo::Result<()> {
        if self.now().play_state == "Playing" {
            self.send(Cmd::PlayPause);
        }
        Ok(())
    }
    async fn play_pause(&self) -> fdo::Result<()> {
        self.send(Cmd::PlayPause);
        Ok(())
    }
    async fn stop(&self) -> fdo::Result<()> {
        if self.now().play_state == "Playing" {
            self.send(Cmd::PlayPause);
        }
        Ok(())
    }
    async fn play(&self) -> fdo::Result<()> {
        if self.now().play_state != "Playing" {
            self.send(Cmd::PlayPause);
        }
        Ok(())
    }
    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let now = self.now();
        let target = (now.position_us + offset.as_micros()).max(0);
        self.send(Cmd::Seek(target as f64 / 1_000_000.0));
        Ok(())
    }
    async fn set_position(&self, _track_id: TrackId, position: Time) -> fdo::Result<()> {
        self.send(Cmd::Seek(position.as_micros().max(0) as f64 / 1_000_000.0));
        Ok(())
    }
    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Ok(())
    }
    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(playback_status(&self.now().play_state))
    }
    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }
    async fn set_loop_status(&self, _loop_status: LoopStatus) -> zbus::Result<()> {
        Ok(())
    }
    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }
    async fn set_rate(&self, _rate: PlaybackRate) -> zbus::Result<()> {
        Ok(())
    }
    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn set_shuffle(&self, _shuffle: bool) -> zbus::Result<()> {
        Ok(())
    }
    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(build_metadata(&self.inner.lock().unwrap()))
    }
    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(volume_frac(&self.now()))
    }
    async fn set_volume(&self, volume: Volume) -> zbus::Result<()> {
        let now = self.now();
        if now.has_volume && now.vol_max > now.vol_min {
            let absolute = now.vol_min + volume.clamp(0.0, 1.0) * (now.vol_max - now.vol_min);
            self.send(Cmd::SetVolume(absolute));
        }
        Ok(())
    }
    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(self.now().position_us))
    }
    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }
    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }
    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(self.now().can_next)
    }
    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(self.now().can_previous)
    }
    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }
    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }
    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(self.now().length_us > 0)
    }
    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
