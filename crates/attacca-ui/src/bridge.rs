//! The QML-facing QObject. All Roon state arrives from the worker thread via
//! `CxxQtThread::queue`; all user actions leave as `Cmd`s on a channel. The
//! QObject itself holds no protocol logic.

use crate::worker::{self, Cmd};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use tokio::sync::mpsc::UnboundedSender;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, connection_state, cxx_name = "connectionState")]
        #[qproperty(QString, core_name, cxx_name = "coreName")]
        #[qproperty(QStringList, zone_list, cxx_name = "zoneList")]
        #[qproperty(i32, zone_index, cxx_name = "zoneIndex")]
        #[qproperty(QString, title)]
        #[qproperty(QString, artist)]
        #[qproperty(QString, album)]
        #[qproperty(QString, art_url, cxx_name = "artUrl")]
        #[qproperty(QString, play_state, cxx_name = "playState")]
        #[qproperty(bool, can_next, cxx_name = "canNext")]
        #[qproperty(bool, can_previous, cxx_name = "canPrevious")]
        #[qproperty(f64, seek_position, cxx_name = "seekPosition")]
        #[qproperty(f64, track_length, cxx_name = "trackLength")]
        #[qproperty(bool, has_volume, cxx_name = "hasVolume")]
        #[qproperty(f64, volume)]
        #[qproperty(f64, volume_min, cxx_name = "volumeMin")]
        #[qproperty(f64, volume_max, cxx_name = "volumeMax")]
        #[qproperty(QString, image_base, cxx_name = "imageBase")]
        #[qproperty(bool, browse_busy, cxx_name = "browseBusy")]
        #[qproperty(bool, shuffle)]
        #[qproperty(bool, auto_radio, cxx_name = "autoRadio")]
        /// "loop" | "loop_one" | "disabled"
        #[qproperty(QString, loop_mode, cxx_name = "loopMode")]
        type App = super::AppRust;

        /// Emitted when the browse view changes to a new list; the UI should
        /// clear its model. `in_search` marks the search hierarchy.
        #[qsignal]
        #[cxx_name = "browseReset"]
        fn browse_reset(self: Pin<&mut App>, title: QString, level: i32, count: i32, in_search: bool);

        /// A JSON array of items to append to the browse model:
        /// [{title, subtitle, imageKey, itemKey, hint}, …]
        #[qsignal]
        #[cxx_name = "browseItems"]
        fn browse_items(self: Pin<&mut App>, items_json: QString);

        /// Transient user-facing notification.
        #[qsignal]
        fn toast(self: Pin<&mut App>, message: QString);

        /// Another launch asked us to come to the front (MPRIS Raise).
        #[qsignal]
        #[cxx_name = "raiseWindow"]
        fn raise_window(self: Pin<&mut App>);

        /// Full queue snapshot for the selected zone as a JSON array:
        /// [{queueItemId, title, subtitle, imageKey, length}, …]
        #[qsignal]
        #[cxx_name = "queueItems"]
        fn queue_items(self: Pin<&mut App>, items_json: QString);

        /// Grouping candidates for the selected zone as a JSON array:
        /// [{outputId, name, zoneName, inCurrent, canGroup}, …]
        #[qsignal]
        #[cxx_name = "groupInfo"]
        fn group_info(self: Pin<&mut App>, outputs_json: QString);

        /// Start discovery + the worker thread. Idempotent; call from QML once.
        #[qinvokable]
        fn start(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "selectZone"]
        fn select_zone(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "playPause"]
        fn play_pause(self: Pin<&mut Self>);

        #[qinvokable]
        fn next(self: Pin<&mut Self>);

        #[qinvokable]
        fn previous(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "changeVolume"]
        fn change_volume(self: Pin<&mut Self>, value: f64);

        #[qinvokable]
        #[cxx_name = "seekTo"]
        fn seek_to(self: Pin<&mut Self>, seconds: f64);

        #[qinvokable]
        #[cxx_name = "browseHome"]
        fn browse_home(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "browseInto"]
        fn browse_into(self: Pin<&mut Self>, item_key: &QString);

        #[qinvokable]
        #[cxx_name = "browseBack"]
        fn browse_back(self: Pin<&mut Self>);

        #[qinvokable]
        fn search(self: Pin<&mut Self>, text: &QString);

        #[qinvokable]
        #[cxx_name = "loadMore"]
        fn load_more(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "playFromHere"]
        fn play_from_here(self: Pin<&mut Self>, queue_item_id: f64);

        #[qinvokable]
        #[cxx_name = "toggleShuffle"]
        fn toggle_shuffle(self: Pin<&mut Self>);

        /// Cycles disabled → loop → loop_one → disabled.
        #[qinvokable]
        #[cxx_name = "cycleLoop"]
        fn cycle_loop(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "toggleRadio"]
        fn toggle_radio(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "requestGroupInfo"]
        fn request_group_info(self: Pin<&mut Self>);

        /// `ids_json` is a JSON array of the output ids that should form the
        /// selected zone after applying.
        #[qinvokable]
        #[cxx_name = "applyGrouping"]
        fn apply_grouping(self: Pin<&mut Self>, ids_json: &QString);
    }

    impl cxx_qt::Threading for App {}
}

use cxx_qt_lib::{QString, QStringList};

pub struct AppRust {
    cmd_tx: Option<UnboundedSender<Cmd>>,

    connection_state: QString,
    core_name: QString,
    zone_list: QStringList,
    zone_index: i32,
    title: QString,
    artist: QString,
    album: QString,
    art_url: QString,
    play_state: QString,
    can_next: bool,
    can_previous: bool,
    seek_position: f64,
    track_length: f64,
    has_volume: bool,
    volume: f64,
    volume_min: f64,
    volume_max: f64,
    image_base: QString,
    browse_busy: bool,
    shuffle: bool,
    auto_radio: bool,
    loop_mode: QString,
}

impl Default for AppRust {
    fn default() -> Self {
        Self {
            cmd_tx: None,
            connection_state: QString::from("starting"),
            core_name: QString::default(),
            zone_list: QStringList::default(),
            zone_index: -1,
            title: QString::default(),
            artist: QString::default(),
            album: QString::default(),
            art_url: QString::default(),
            play_state: QString::from("Stopped"),
            can_next: false,
            can_previous: false,
            seek_position: 0.0,
            track_length: 0.0,
            has_volume: false,
            volume: 0.0,
            volume_min: 0.0,
            volume_max: 100.0,
            image_base: QString::default(),
            browse_busy: false,
            shuffle: false,
            auto_radio: false,
            loop_mode: QString::from("disabled"),
        }
    }
}

impl qobject::App {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.cmd_tx.is_some() {
            return;
        }
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let qt_thread = self.qt_thread();
        let worker_tx = tx.clone();
        self.as_mut().rust_mut().cmd_tx = Some(tx);
        std::thread::spawn(move || worker::run(qt_thread, worker_tx, rx));
    }

    fn send(&self, cmd: Cmd) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }

    pub fn select_zone(self: Pin<&mut Self>, index: i32) {
        if index >= 0 {
            self.send(Cmd::SelectZone(index as usize));
        }
    }

    pub fn play_pause(self: Pin<&mut Self>) {
        self.send(Cmd::PlayPause);
    }

    pub fn next(self: Pin<&mut Self>) {
        self.send(Cmd::Next);
    }

    pub fn previous(self: Pin<&mut Self>) {
        self.send(Cmd::Previous);
    }

    pub fn change_volume(self: Pin<&mut Self>, value: f64) {
        self.send(Cmd::SetVolume(value));
    }

    pub fn seek_to(self: Pin<&mut Self>, seconds: f64) {
        self.send(Cmd::Seek(seconds));
    }

    pub fn browse_home(self: Pin<&mut Self>) {
        self.send(Cmd::BrowseHome);
    }

    pub fn browse_into(self: Pin<&mut Self>, item_key: &QString) {
        self.send(Cmd::BrowseInto(item_key.to_string()));
    }

    pub fn browse_back(self: Pin<&mut Self>) {
        self.send(Cmd::BrowseBack);
    }

    pub fn search(self: Pin<&mut Self>, text: &QString) {
        let text = text.to_string();
        if !text.trim().is_empty() {
            self.send(Cmd::Search(text));
        }
    }

    pub fn load_more(self: Pin<&mut Self>) {
        self.send(Cmd::LoadMore);
    }

    pub fn play_from_here(self: Pin<&mut Self>, queue_item_id: f64) {
        self.send(Cmd::PlayFromHere(queue_item_id as u64));
    }

    pub fn toggle_shuffle(self: Pin<&mut Self>) {
        let next = !*self.shuffle();
        self.send(Cmd::SetShuffle(next));
    }

    pub fn cycle_loop(self: Pin<&mut Self>) {
        let next = match self.loop_mode().to_string().as_str() {
            "disabled" => "loop",
            "loop" => "loop_one",
            _ => "disabled",
        };
        self.send(Cmd::SetLoop(next.to_owned()));
    }

    pub fn toggle_radio(self: Pin<&mut Self>) {
        let next = !*self.auto_radio();
        self.send(Cmd::SetRadio(next));
    }

    pub fn request_group_info(self: Pin<&mut Self>) {
        self.send(Cmd::GroupInfo);
    }

    pub fn apply_grouping(self: Pin<&mut Self>, ids_json: &QString) {
        match serde_json::from_str::<Vec<String>>(&ids_json.to_string()) {
            Ok(ids) if !ids.is_empty() => self.send(Cmd::ApplyGrouping(ids)),
            Ok(_) => {}
            Err(e) => tracing::warn!("applyGrouping: bad ids payload: {e}"),
        }
    }
}
