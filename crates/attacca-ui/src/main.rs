//! Attacca desktop UI: QML shell over the Roon worker.

pub mod bridge;
pub mod mpris;
pub mod worker;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .init();

    // Single instance: a second launch raises the running window and exits.
    // Two instances would share one Roon extension identity, and the Core
    // resets connections when the same identity connects twice.
    // ATTACCA_ALLOW_MULTI=1 bypasses the check for development.
    if std::env::var_os("ATTACCA_ALLOW_MULTI").is_none() {
        let running = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .is_ok_and(|rt| rt.block_on(mpris::instance_already_running()));
        if running {
            println!("Attacca is already running — raised the existing window.");
            return;
        }
    }

    // Material dark is our look; allow the user to override via env.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        std::env::set_var("QT_QUICK_CONTROLS_STYLE", "Material");
    }

    let mut app = QGuiApplication::new();
    // Ties the Wayland app_id to attacca.desktop (launcher icon, window
    // grouping, MPRIS DesktopEntry).
    QGuiApplication::set_desktop_file_name(&QString::from("attacca"));
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/org/attacca/qml/Main.qml"));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
