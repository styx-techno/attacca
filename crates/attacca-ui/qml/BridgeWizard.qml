import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import org.attacca

// Guided Roon Bridge setup: makes this computer a Roon zone by installing
// Roon's own Bridge per-user (downloaded from Roon's servers on request —
// bundling it is not permitted), and manages the per-device choice between
// exclusive ALSA access and mixing into the desktop through PipeWire.
Dialog {
    id: dlg
    modal: true
    title: "Play to this computer"
    anchors.centerIn: parent
    width: Math.min(560, (parent ? parent.width : 640) - 48)
    contentHeight: Math.min(wizCol.implicitHeight,
                            (parent ? parent.height : 720) - 200)
    standardButtons: Dialog.Close

    readonly property color textMain: "#e8e8ee"
    readonly property color textDim: "#8a8b96"

    readonly property bool installed: setup.installedVersion !== ""
    // Another Bridge (typically a system package running as root) owns this
    // machine's RAATServer. A second install would silently attach to it and
    // add nothing, so the wizard stands down instead.
    readonly property bool foreignBridge: setup.raatAlive && !setup.serviceActive
                                          && !installed

    onOpened: {
        removeBtn.armed = false
        setup.refresh()
        setup.scanDevices()
    }

    BridgeSetup {
        id: setup

        // The result of an operation survives closing and reopening the
        // dialog; only starting the next operation clears it.
        onBusyChanged: if (busy) resultLabel.text = ""

        onDevices: json => {
            deviceModel.clear()
            // Leftover settings files without an install would render a
            // device list under the "not installed" pane.
            if (setup.installedVersion === "")
                return
            for (const d of JSON.parse(json))
                deviceModel.append({ deviceId: d.id, name: d.name, mode: d.mode })
        }

        onOpFinished: (ok, message) => {
            resultLabel.ok = ok
            resultLabel.text = message
        }
    }

    ListModel { id: deviceModel }

    ScrollView {
        id: scroller
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth

        ColumnLayout {
            id: wizCol
            width: scroller.availableWidth
            spacing: 10

            Label {
                visible: setup.sandboxed
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: dlg.textMain
                text: "The Flatpak can't set this up: the sandbox is not allowed "
                      + "to install a service on the host. Everything else works "
                      + "without it — for playback on this computer, install "
                      + "Attacca natively or set up Roon Bridge manually."
            }

            Label {
                visible: !setup.sandboxed && dlg.foreignBridge
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: dlg.textMain
                text: "Roon Bridge already runs on this computer as a system "
                      + "service — its devices are ready to enable in Roon "
                      + "Settings → Audio. Attacca only manages its own per-user "
                      + "install; for the desktop-mix option, remove the system "
                      + "install first, then set it up here."
            }

            Label {
                visible: !setup.sandboxed && !dlg.foreignBridge && !dlg.installed
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: dlg.textMain
                text: "Roon can play to this computer once Roon Bridge runs on "
                      + "it. Attacca downloads Roon Bridge from Roon's servers "
                      + "and runs it as a service for your user only — no "
                      + "administrator access needed."
            }

            Button {
                visible: !setup.sandboxed && !dlg.foreignBridge && !dlg.installed
                enabled: !setup.busy
                text: "Download and install"
                highlighted: true
                onClicked: setup.install()
            }

            GridLayout {
                visible: dlg.installed
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 12
                rowSpacing: 2

                Label { text: "Installed"; color: dlg.textDim; font.pixelSize: 12 }
                Label {
                    text: setup.installedVersion
                    color: dlg.textMain
                    font.pixelSize: 12
                }
                Label { text: "Service"; color: dlg.textDim; font.pixelSize: 12 }
                Label {
                    text: setup.serviceActive
                          ? (setup.raatAlive ? "running" : "starting…")
                          : "stopped"
                    color: setup.serviceActive ? dlg.textMain : Material.accent
                    font.pixelSize: 12
                }
            }

            RowLayout {
                visible: dlg.installed
                Layout.fillWidth: true
                Layout.topMargin: 4

                Label {
                    text: "Enabled devices"
                    color: dlg.textMain
                    font.pixelSize: 13
                    font.bold: true
                    Layout.fillWidth: true
                }
                Button {
                    flat: true
                    text: "Refresh"
                    enabled: !setup.busy
                    onClicked: { setup.refresh(); setup.scanDevices() }
                }
            }

            Label {
                visible: dlg.installed && deviceModel.count === 0
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: dlg.textDim
                font.pixelSize: 12
                text: "None yet. In Roon Settings → Audio (from Roon on your "
                      + "phone or another computer), enable a device listed "
                      + "under this computer's name — it appears here afterwards."
            }

            Repeater {
                model: deviceModel

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Label {
                        Layout.fillWidth: true
                        text: model.name
                        color: dlg.textMain
                        font.pixelSize: 13
                        elide: Text.ElideRight
                    }
                    ComboBox {
                        Layout.preferredWidth: 220
                        model: ["Exclusive (bit-perfect)", "Desktop mix (PipeWire)"]
                        currentIndex: mode === "pipewire" ? 1 : 0
                        enabled: !setup.busy
                                && (setup.pipewireOk || mode === "pipewire")
                        onActivated: index => {
                            const want = index === 1 ? "pipewire" : "exclusive"
                            if (want !== mode)
                                setup.setDeviceMode(deviceId, want)
                        }
                    }
                }
            }

            Label {
                visible: dlg.installed && !setup.pipewireOk
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: dlg.textDim
                font.pixelSize: 12
                text: "Desktop mix needs the pipewire-alsa package, which is "
                      + "not installed."
            }

            RowLayout {
                visible: dlg.installed
                spacing: 8

                Button {
                    flat: true
                    enabled: !setup.busy
                    text: "Update / reinstall"
                    onClicked: setup.install()
                }
                Button {
                    id: removeBtn
                    flat: true
                    enabled: !setup.busy
                    property bool armed: false
                    text: armed ? "Click again to remove" : "Remove"
                    onClicked: {
                        if (armed) {
                            armed = false
                            setup.uninstall()
                        } else {
                            armed = true
                            disarmTimer.restart()
                        }
                    }
                    Timer {
                        id: disarmTimer
                        interval: 3000
                        onTriggered: removeBtn.armed = false
                    }
                }
            }

            ColumnLayout {
                visible: setup.busy
                Layout.fillWidth: true
                spacing: 6

                ProgressBar {
                    Layout.fillWidth: true
                    indeterminate: setup.progress < 0
                    value: Math.max(0, setup.progress)
                }
                Label {
                    text: setup.statusText
                    color: dlg.textDim
                    font.pixelSize: 12
                }
            }

            Label {
                id: resultLabel
                property bool ok: true
                visible: text !== ""
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: ok ? dlg.textDim : Material.accent
                font.pixelSize: 12
            }

            Label {
                visible: !setup.sandboxed && !dlg.installed && !dlg.foreignBridge
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: dlg.textDim
                font.pixelSize: 12
                text: "Roon Bridge is Roon Labs software, fetched from "
                      + "download.roonlabs.net and subject to Roon's terms. "
                      + "Attacca does not bundle or redistribute it."
            }
        }
    }
}
