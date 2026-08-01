use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("org.attacca").qml_file("qml/Main.qml"))
        .files(["src/bridge.rs"])
        .build();
}
