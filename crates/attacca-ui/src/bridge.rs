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
        self.as_mut().rust_mut().cmd_tx = Some(tx);
        std::thread::spawn(move || worker::run(qt_thread, rx));
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
}
