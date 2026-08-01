import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import org.attacca

ApplicationWindow {
    id: root
    visible: true
    width: 960
    height: 720
    minimumWidth: 400
    minimumHeight: 620
    title: "Attacca"
    color: "#101014"
    Material.theme: Material.Dark
    Material.accent: "#e8735a"
    Material.background: "#101014"

    property string browseTitle: ""
    property int browseLevel: 0
    property int browseCount: 0
    property bool inSearch: false
    property bool gridMode: false

    App {
        id: app
        Component.onCompleted: app.start()

        onBrowseReset: (title, level, count, isSearch) => {
            browseModel.clear()
            root.browseTitle = title
            root.browseLevel = level
            root.browseCount = count
            root.inSearch = isSearch
        }

        onBrowseItems: json => {
            const items = JSON.parse(json)
            const firstBatch = browseModel.count === 0
            for (const it of items)
                browseModel.append(it)
            if (firstBatch && items.length > 0) {
                let withArt = 0
                for (const it of items)
                    if (it.imageKey !== "")
                        withArt++
                root.gridMode = app.imageBase.toString() !== ""
                        && withArt / items.length >= 0.5
            }
        }

        onToast: message => {
            toastLabel.text = message
            toastTimer.restart()
        }
    }

    ListModel { id: browseModel }

    function fmt(s) {
        s = Math.max(0, Math.round(s))
        var m = Math.floor(s / 60)
        var sec = s % 60
        return m + ":" + (sec < 10 ? "0" : "") + sec
    }

    function thumb(key, px) {
        return key === "" ? ""
                          : app.imageBase + key + "?scale=fit&width=" + px
                            + "&height=" + px + "&format=image/jpeg"
    }

    function maybeLoadMore() {
        if (browseModel.count < root.browseCount && !app.browseBusy)
            app.loadMore()
    }

    // Flickable's built-in wheel handling is tuned for touch physics and
    // crawls on a desktop mouse wheel; scroll a real step per notch instead.
    // Trust angleDelta: on Wayland a mouse wheel ALSO carries a tiny
    // pixelDelta (~15px per notch from libinput), which must not win.
    // Full notches glide via `anim` (the target accumulates, so fast spinning
    // keeps momentum); sub-notch deltas from touchpads/free-spin wheels are
    // applied directly to preserve their native smoothness.
    function wheelScroll(view, anim, event) {
        view.cancelFlick()
        const maxY = view.originY + Math.max(0, view.contentHeight - view.height)
        const clamp = y => Math.max(view.originY, Math.min(maxY, y))
        const dy = event.angleDelta.y !== 0 ? event.angleDelta.y / 120 * 240
                                            : event.pixelDelta.y
        if (Math.abs(event.angleDelta.y) >= 120) {
            const base = anim.running ? anim.to : view.contentY
            anim.stop()
            anim.from = view.contentY
            anim.to = clamp(base - dy)
            anim.start()
        } else {
            anim.stop()
            view.contentY = clamp(view.contentY - dy)
        }
        event.accepted = true
    }

    footer: TabBar {
        id: tabs
        visible: app.connectionState === "connected"
        Material.background: "#16161c"

        TabButton { text: "Now Playing" }
        TabButton { text: "Browse" }
    }

    StackLayout {
        anchors.fill: parent
        visible: app.connectionState === "connected"
        currentIndex: tabs.currentIndex

        // ───────────────────────────── Now Playing ─────────────────────────
        ColumnLayout {
            Layout.margins: 24
            spacing: 14

            ComboBox {
                Layout.fillWidth: true
                Layout.margins: 24
                Layout.bottomMargin: 0
                model: app.zoneList
                currentIndex: app.zoneIndex
                onActivated: index => app.selectZone(index)
            }

            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.margins: 24
                Layout.topMargin: 8
                Layout.bottomMargin: 8

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
                Layout.leftMargin: 24
                Layout.rightMargin: 24
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
                Layout.leftMargin: 24
                Layout.rightMargin: 24
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
                Layout.leftMargin: 24
                Layout.rightMargin: 24
                Layout.bottomMargin: 16
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

        // ─────────────────────────────── Browse ────────────────────────────
        ColumnLayout {
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                Layout.margins: 16
                Layout.bottomMargin: 0
                spacing: 8

                ToolButton {
                    text: "‹"
                    font.pixelSize: 22
                    visible: root.browseLevel > 0 || root.inSearch
                    onClicked: (root.inSearch && root.browseLevel === 0)
                               ? app.browseHome()
                               : app.browseBack()
                }
                ToolButton {
                    text: "⌂"
                    onClicked: app.browseHome()
                }
                Label {
                    Layout.fillWidth: true
                    text: root.browseTitle
                    font.pixelSize: 18
                    font.bold: true
                    elide: Text.ElideRight
                }
                BusyIndicator {
                    implicitWidth: 28
                    implicitHeight: 28
                    running: app.browseBusy
                    visible: app.browseBusy
                }
            }

            TextField {
                id: searchField
                Layout.fillWidth: true
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                placeholderText: "Search library, TIDAL, Qobuz…"
                onAccepted: app.search(text)
            }

            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                visible: root.gridMode

                GridView {
                id: grid
                anchors.fill: parent
                clip: true
                model: root.gridMode ? browseModel : null
                cellWidth: Math.floor(width / Math.max(2, Math.floor(width / 176)))
                cellHeight: cellWidth + 46
                onAtYEndChanged: if (atYEnd) maybeLoadMore()
                onDraggingChanged: if (dragging) gridAnim.stop()

                ScrollBar.vertical: ScrollBar {}

                delegate: Item {
                    width: grid.cellWidth
                    height: grid.cellHeight

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 8
                        spacing: 4

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: width
                            radius: 6
                            color: "#191920"

                            Image {
                                anchors.fill: parent
                                source: thumb(model.imageKey, 320)
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            text: model.title
                            font.pixelSize: 13
                            elide: Text.ElideRight
                        }
                        Label {
                            Layout.fillWidth: true
                            text: model.subtitle
                            font.pixelSize: 11
                            color: "#7c7d88"
                            elide: Text.ElideRight
                            visible: model.subtitle !== ""
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: if (model.itemKey !== "") app.browseInto(model.itemKey)
                    }
                }
                }

                NumberAnimation {
                    id: gridAnim
                    target: grid
                    property: "contentY"
                    duration: 160
                    easing.type: Easing.OutCubic
                }

                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.NoButton
                    onWheel: wheel => root.wheelScroll(grid, gridAnim, wheel)
                }
            }

            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: !root.gridMode

                ListView {
                id: list
                anchors.fill: parent
                clip: true
                model: root.gridMode ? null : browseModel
                onAtYEndChanged: if (atYEnd) maybeLoadMore()
                onDraggingChanged: if (dragging) listAnim.stop()

                ScrollBar.vertical: ScrollBar {}

                delegate: ItemDelegate {
                    width: list.width
                    onClicked: if (model.itemKey !== "") app.browseInto(model.itemKey)

                    contentItem: RowLayout {
                        spacing: 12

                        Rectangle {
                            implicitWidth: 44
                            implicitHeight: 44
                            radius: 4
                            color: "#191920"
                            visible: model.imageKey !== ""

                            Image {
                                anchors.fill: parent
                                source: thumb(model.imageKey, 88)
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Label {
                                Layout.fillWidth: true
                                text: model.title
                                font.pixelSize: 14
                                elide: Text.ElideRight
                            }
                            Label {
                                Layout.fillWidth: true
                                text: model.subtitle
                                font.pixelSize: 12
                                color: "#7c7d88"
                                elide: Text.ElideRight
                                visible: model.subtitle !== ""
                            }
                        }

                        Label {
                            text: "›"
                            color: "#4a4b55"
                            font.pixelSize: 18
                            visible: model.hint === "list" || model.hint === "action_list"
                        }
                    }
                }
            }

                NumberAnimation {
                    id: listAnim
                    target: list
                    property: "contentY"
                    duration: 160
                    easing.type: Easing.OutCubic
                }

                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.NoButton
                    onWheel: wheel => root.wheelScroll(list, listAnim, wheel)
                }
            }
        }
    }

    // ─────────────────────────── Connection overlay ────────────────────────
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

    // ─────────────────────────────── Toast ─────────────────────────────────
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 64
        radius: 8
        color: "#2a2a33"
        opacity: toastTimer.running ? 0.95 : 0
        visible: opacity > 0
        width: toastLabel.implicitWidth + 32
        height: toastLabel.implicitHeight + 20

        Behavior on opacity { NumberAnimation { duration: 200 } }

        Label {
            id: toastLabel
            anchors.centerIn: parent
            color: "#e8e8ee"
        }

        Timer {
            id: toastTimer
            interval: 2500
        }
    }
}
