import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import org.attacca

ApplicationWindow {
    id: root
    visible: true
    width: 460
    height: 760
    minimumWidth: 380
    minimumHeight: 620
    title: "Attacca"
    color: "#101014"
    Material.theme: Material.Dark
    Material.accent: "#e8735a"
    Material.background: "#101014"

    App {
        id: app
        Component.onCompleted: app.start()
    }

    function fmt(s) {
        s = Math.max(0, Math.round(s))
        var m = Math.floor(s / 60)
        var sec = s % 60
        return m + ":" + (sec < 10 ? "0" : "") + sec
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 14
        visible: app.connectionState === "connected"

        ComboBox {
            Layout.fillWidth: true
            model: app.zoneList
            currentIndex: app.zoneIndex
            onActivated: index => app.selectZone(index)
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            Rectangle {
                id: artFrame
                anchors.centerIn: parent
                width: Math.min(parent.width, parent.height)
                height: width
                radius: 10
                color: "#191920"
                border.color: "#26262e"

                Image {
                    anchors.fill: parent
                    anchors.margins: 1
                    source: app.artUrl
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    visible: status === Image.Ready
                }

                Text {
                    anchors.centerIn: parent
                    text: "♪"
                    color: "#33343c"
                    font.pixelSize: artFrame.width / 4
                    visible: app.artUrl.toString() === ""
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Label {
                Layout.fillWidth: true
                text: app.title
                font.pixelSize: 22
                font.bold: true
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignHCenter
            }
            Label {
                Layout.fillWidth: true
                text: app.artist
                font.pixelSize: 15
                color: "#b9bac4"
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignHCenter
            }
            Label {
                Layout.fillWidth: true
                text: app.album
                font.pixelSize: 13
                color: "#7c7d88"
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignHCenter
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            Slider {
                id: seek
                Layout.fillWidth: true
                from: 0
                to: Math.max(1, app.trackLength)
                enabled: app.trackLength > 0
                onPressedChanged: if (!pressed) app.seekTo(value)

                Binding on value {
                    when: !seek.pressed
                    value: app.seekPosition
                }
            }

            RowLayout {
                Layout.fillWidth: true

                Label {
                    text: fmt(seek.pressed ? seek.value : app.seekPosition)
                    font.pixelSize: 12
                    color: "#7c7d88"
                }
                Item { Layout.fillWidth: true }
                Label {
                    text: fmt(app.trackLength)
                    font.pixelSize: 12
                    color: "#7c7d88"
                }
            }
        }

        RowLayout {
            Layout.alignment: Qt.AlignHCenter
            spacing: 28

            RoundButton {
                flat: true
                text: "◀◀"
                font.pixelSize: 16
                enabled: app.canPrevious
                onClicked: app.previous()
            }

            RoundButton {
                implicitWidth: 76
                implicitHeight: 76
                text: app.playState === "Playing" ? "▮▮" : "▶"
                font.pixelSize: 24
                Material.background: root.Material.accent
                onClicked: app.playPause()
            }

            RoundButton {
                flat: true
                text: "▶▶"
                font.pixelSize: 16
                enabled: app.canNext
                onClicked: app.next()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            visible: app.hasVolume
            spacing: 12

            Label {
                text: "♫"
                color: "#7c7d88"
            }
            Slider {
                id: volume
                Layout.fillWidth: true
                from: app.volumeMin
                to: app.volumeMax
                onMoved: app.changeVolume(value)

                Binding on value {
                    when: !volume.pressed
                    value: app.volume
                }
            }
            Label {
                text: Math.round(volume.pressed ? volume.value : app.volume)
                color: "#7c7d88"
                font.pixelSize: 12
            }
        }
    }

    ColumnLayout {
        anchors.centerIn: parent
        spacing: 18
        visible: app.connectionState !== "connected"

        BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            running: visible
        }
        Label {
            Layout.alignment: Qt.AlignHCenter
            horizontalAlignment: Text.AlignHCenter
            color: "#b9bac4"
            text: app.connectionState === "pairing"
                  ? "Waiting for approval —\nenable “Attacca” in Roon Settings → Extensions"
                  : "Looking for your Roon Core… (" + app.connectionState + ")"
        }
    }
}
