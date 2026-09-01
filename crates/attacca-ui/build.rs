use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("org.attacca")
            .qml_file("qml/Main.qml")
            .qml_file("qml/BridgeWizard.qml"),
    )
    .files(["src/bridge.rs", "src/bridge_setup.rs"])
        .qrc("icons.qrc")
        .build();
}
