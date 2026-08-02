import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Effects
import QtQuick.Layouts
import org.attacca

// Layout follows the convention Roon's desktop app established — sidebar
// navigation, a persistent transport bar along the bottom, and the queue as a
// right-hand panel — so muscle memory carries over. The icons are our own.
ApplicationWindow {
    id: root
    visible: true
    width: 1180
    height: 780
    minimumWidth: 640
    minimumHeight: 560
    title: "Attacca"
    color: "#0f0f13"
    Material.theme: Material.Dark
    Material.accent: "#e8735a"
    Material.background: "#0f0f13"

    readonly property color panelColor: "#141419"
    readonly property color railColor: "#0b0b0e"
    readonly property color lineColor: "#232329"
    readonly property color textDim: "#8a8b96"
    readonly property color textOff: "#55565f"

    property string browseTitle: ""
    property int browseLevel: 0
    property int browseCount: 0
    property bool inSearch: false
    property bool gridMode: false
    property string activeNav: ""
    property bool queueOpen: false
    property bool npOpen: false

    // At the browse root the sidebar already lists every section, so repeating
    // it in the content area is pure duplication — show a prompt instead.
    readonly property bool atRoot: browseLevel === 0 && !inSearch

    Settings {
        location: StandardPaths.writableLocation(StandardPaths.GenericConfigLocation)
                  + "/attacca/ui.conf"
        property alias windowWidth: root.width
        property alias windowHeight: root.height
        property alias queueOpen: root.queueOpen
    }

    Shortcut {
        sequence: "Space"
        enabled: !searchField.activeFocus
        onActivated: app.playPause()
    }
    Shortcut { sequence: "Ctrl+Right"; onActivated: app.next() }
    Shortcut { sequence: "Ctrl+Left"; onActivated: app.previous() }
    Shortcut {
        sequence: "Ctrl+F"
        onActivated: {
            root.npOpen = false
            searchField.forceActiveFocus()
            searchField.selectAll()
        }
    }
    Shortcut {
        sequence: "Ctrl+Q"
        onActivated: root.queueOpen = !root.queueOpen
    }
    Shortcut {
        sequence: "Escape"
        enabled: searchField.activeFocus || root.npOpen
                 || root.browseLevel > 0 || root.inSearch
        onActivated: {
            if (root.npOpen)
                root.npOpen = false
            else if (searchField.activeFocus)
                searchField.focus = false
            else if (root.inSearch && root.browseLevel === 0)
                app.browseHome()
            else
                app.browseBack()
        }
    }

    App {
        id: app
        Component.onCompleted: app.start()

        onBrowseReset: (title, level, count, isSearch) => {
            browseModel.clear()
            root.browseTitle = title
            root.browseLevel = level
            root.browseCount = count
            root.inSearch = isSearch
            if (level === 0 && !isSearch)
                root.activeNav = ""
        }

        onBrowseItems: json => {
            const items = JSON.parse(json)
            const firstBatch = browseModel.count === 0
            for (const it of items)
                browseModel.append(it)

            // The browse root IS Roon's own top-level menu, so the sidebar is
            // populated from it rather than from a hardcoded list that would
            // drift from whatever the core actually offers.
            if (root.browseLevel === 0 && !root.inSearch && firstBatch) {
                navModel.clear()
                for (const it of items)
                    if (it.itemKey !== "")
                        navModel.append(it)
            }

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

        onQueueItems: json => {
            const items = JSON.parse(json)
            queueModel.clear()
            for (const it of items)
                queueModel.append(it)
        }

        onRaiseWindow: {
            root.show()
            root.raise()
            root.requestActivate()
        }

        onGroupInfo: json => {
            const outputs = JSON.parse(json)
            groupModel.clear()
            for (const o of outputs)
                groupModel.append({
                    outputId: o.outputId,
                    name: o.name,
                    zoneName: o.zoneName,
                    canGroup: o.canGroup,
                    checked: o.inCurrent
                })
            groupDialog.open()
        }
    }

    ListModel { id: browseModel }
    ListModel { id: queueModel }
    ListModel { id: groupModel }
    ListModel { id: navModel }

    // Sections Roon's own app has that its extension API does not expose. They
    // are shown rather than hidden so the absence reads as a documented limit
    // instead of a missing feature.
    ListModel {
        id: lockedNavModel
        ListElement {
            title: "Home"
            icon: "qrc:/icons/home.svg"
            why: "Roon's home screen — Daily Mixes and recommendations — is not exposed to extensions."
        }
        ListElement {
            title: "Discover"
            icon: "qrc:/icons/album.svg"
            why: "Discover is not exposed to extensions by Roon's API."
        }
        ListElement {
            title: "Settings"
            icon: "qrc:/icons/settings.svg"
            why: "Roon's settings, DSP engine and signal path are not reachable through the extension API."
        }
    }

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

    // Roon's own browse root already offers some of the sections we list as
    // unavailable (Settings, notably, exists as a limited hierarchy). Showing
    // both a working and a locked copy reads as a bug, so the locked entry
    // yields. Touching navModel.count keeps this binding live.
    function navHas(title) {
        for (let i = 0; i < navModel.count; i++)
            if (navModel.get(i).title === title)
                return true
        return false
    }

    function navIcon(title) {
        const t = title.toLowerCase()
        if (t.indexOf("artist") >= 0) return "qrc:/icons/artist.svg"
        if (t.indexOf("album") >= 0) return "qrc:/icons/album.svg"
        if (t.indexOf("playlist") >= 0) return "qrc:/icons/playlist.svg"
        if (t.indexOf("radio") >= 0) return "qrc:/icons/radio.svg"
        if (t.indexOf("search") >= 0) return "qrc:/icons/search.svg"
        if (t.indexOf("setting") >= 0) return "qrc:/icons/settings.svg"
        return "qrc:/icons/library.svg"
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

    // ══════════════════════════════ Main shell ═════════════════════════════
    Item {
        anchors.fill: parent
        visible: app.connectionState === "connected"

        // ───────────────────────────── Sidebar ─────────────────────────────
        Rectangle {
            id: sidebar
            anchors { left: parent.left; top: parent.top; bottom: transport.top }
            width: 224
            color: root.railColor

            Rectangle {
                anchors { right: parent.right; top: parent.top; bottom: parent.bottom }
                width: 1
                color: root.lineColor
            }

            ColumnLayout {
                anchors.fill: parent
                anchors.rightMargin: 1
                spacing: 0

                // Wordmark + which core we are attached to
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.margins: 18
                    Layout.bottomMargin: 10
                    spacing: 1

                    Label {
                        text: "Attacca"
                        font.pixelSize: 19
                        font.bold: true
                        font.letterSpacing: 0.5
                    }
                    Label {
                        Layout.fillWidth: true
                        text: app.coreName
                        font.pixelSize: 11
                        color: root.textDim
                        elide: Text.ElideRight
                        visible: text !== ""
                    }
                }

                ListView {
                    id: nav
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: navModel
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}

                    delegate: ItemDelegate {
                        width: nav.width
                        height: 38
                        highlighted: root.activeNav === model.title
                        onClicked: {
                            root.activeNav = model.title
                            root.npOpen = false
                            app.browseInto(model.itemKey)
                        }

                        // Roon marks the active section with a bar down the
                        // leading edge rather than a filled row.
                        Rectangle {
                            anchors { left: parent.left; top: parent.top; bottom: parent.bottom }
                            width: 3
                            color: root.Material.accent
                            visible: root.activeNav === model.title
                        }

                        contentItem: RowLayout {
                            spacing: 12

                            Image {
                                source: root.navIcon(model.title)
                                sourceSize.width: 17
                                sourceSize.height: 17
                                opacity: root.activeNav === model.title ? 0.95 : 0.5
                            }
                            Label {
                                Layout.fillWidth: true
                                text: model.title
                                font.pixelSize: 13
                                elide: Text.ElideRight
                                color: root.activeNav === model.title ? "#f0f0f5" : "#b9bac4"
                            }
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.leftMargin: 16
                    Layout.rightMargin: 16
                    height: 1
                    color: root.lineColor
                    visible: navModel.count > 0
                }

                Repeater {
                    model: lockedNavModel

                    ItemDelegate {
                        Layout.fillWidth: true
                        visible: !root.navHas(model.title)
                        height: visible ? 36 : 0
                        enabled: false
                        opacity: 1.0
                        hoverEnabled: true

                        // enabled:false kills hover, so the explanation rides
                        // on a HoverHandler instead of the delegate.
                        HoverHandler { id: lockHover; enabled: true }
                        ToolTip.visible: lockHover.hovered
                        ToolTip.text: model.why
                        ToolTip.delay: 300

                        contentItem: RowLayout {
                            spacing: 12

                            Image {
                                source: model.icon
                                sourceSize.width: 17
                                sourceSize.height: 17
                                opacity: 0.22
                            }
                            Label {
                                Layout.fillWidth: true
                                text: model.title
                                font.pixelSize: 13
                                elide: Text.ElideRight
                                color: root.textOff
                            }
                            Image {
                                source: "qrc:/icons/lock.svg"
                                sourceSize.width: 12
                                sourceSize.height: 12
                                opacity: 0.28
                            }
                        }
                    }
                }

                Item { Layout.preferredHeight: 10 }
            }
        }

        // ──────────────────────────── Main column ──────────────────────────
        Item {
            id: mainArea
            anchors {
                left: sidebar.right
                right: queuePanel.visible ? queuePanel.left : parent.right
                top: parent.top
                bottom: transport.top
            }

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                // Top bar: history, breadcrumb, search
                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 58

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 14
                        anchors.rightMargin: 14
                        spacing: 6

                        ToolButton {
                            icon.source: "qrc:/icons/back.svg"
                            icon.width: 20
                            icon.height: 20
                            icon.color: enabled ? "#c9cad4" : root.textOff
                            enabled: root.browseLevel > 0 || root.inSearch
                            ToolTip.visible: hovered && enabled
                            ToolTip.text: "Back"
                            onClicked: (root.inSearch && root.browseLevel === 0)
                                       ? app.browseHome() : app.browseBack()
                        }
                        ToolButton {
                            icon.source: "qrc:/icons/home.svg"
                            icon.width: 18
                            icon.height: 18
                            icon.color: "#c9cad4"
                            ToolTip.visible: hovered
                            ToolTip.text: "Library root"
                            onClicked: app.browseHome()
                        }

                        Label {
                            Layout.fillWidth: true
                            Layout.leftMargin: 6
                            text: root.browseTitle
                            font.pixelSize: 17
                            font.bold: true
                            elide: Text.ElideRight
                        }

                        BusyIndicator {
                            implicitWidth: 24
                            implicitHeight: 24
                            running: app.browseBusy
                            visible: app.browseBusy
                        }

                        TextField {
                            id: searchField
                            Layout.preferredWidth: Math.min(300, mainArea.width * 0.34)
                            leftPadding: 34
                            placeholderText: "Search"
                            onAccepted: app.search(text)

                            Image {
                                anchors.left: parent.left
                                anchors.leftMargin: 8
                                anchors.verticalCenter: parent.verticalCenter
                                source: "qrc:/icons/search.svg"
                                sourceSize.width: 16
                                sourceSize.height: 16
                                opacity: 0.45
                            }
                        }
                    }

                    Rectangle {
                        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                        height: 1
                        color: root.lineColor
                    }
                }

                // Root prompt — the sidebar carries the sections themselves.
                // Centred by anchors, not layout alignment: the ColumnLayout
                // collapsed to its widest child and stranded this at the left.
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: root.atRoot

                    ColumnLayout {
                        anchors.centerIn: parent
                        spacing: 10

                        Image {
                            Layout.alignment: Qt.AlignHCenter
                            source: "qrc:/icons/library.svg"
                            sourceSize.width: 44
                            sourceSize.height: 44
                            opacity: 0.13
                        }
                        Label {
                            Layout.alignment: Qt.AlignHCenter
                            text: "Choose a section to browse"
                            font.pixelSize: 15
                            color: root.textDim
                        }
                        Label {
                            Layout.alignment: Qt.AlignHCenter
                            text: "or press Ctrl+F to search your library, TIDAL and Qobuz"
                            font.pixelSize: 12
                            color: root.textOff
                        }
                    }
                }

                // Artwork grid
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.leftMargin: 14
                    Layout.rightMargin: 6
                    visible: root.gridMode && !root.atRoot

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
                            scale: tileHover.hovered ? 1.03 : 1.0
                            z: tileHover.hovered ? 1 : 0

                            Behavior on scale { NumberAnimation { duration: 120 } }

                            HoverHandler {
                                id: tileHover
                                cursorShape: Qt.PointingHandCursor
                            }

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 8
                                spacing: 4

                                Rectangle {
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: width
                                    radius: 4
                                    color: "#191920"

                                    Image {
                                        anchors.fill: parent
                                        source: thumb(model.imageKey, 320)
                                        fillMode: Image.PreserveAspectCrop
                                        asynchronous: true
                                    }

                                    // Roon reveals a play affordance on hover
                                    Rectangle {
                                        anchors.fill: parent
                                        color: "#000000"
                                        opacity: tileHover.hovered ? 0.35 : 0
                                        Behavior on opacity { NumberAnimation { duration: 120 } }
                                    }
                                    Image {
                                        anchors.centerIn: parent
                                        source: "qrc:/icons/play.svg"
                                        sourceSize.width: 40
                                        sourceSize.height: 40
                                        opacity: tileHover.hovered ? 0.92 : 0
                                        Behavior on opacity { NumberAnimation { duration: 120 } }
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
                                    color: root.textDim
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

                // Track / section list
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.leftMargin: 8
                    Layout.rightMargin: 6
                    visible: !root.gridMode && !root.atRoot

                    ListView {
                        id: list
                        // Full-bleed rows on a wide monitor strand the chevron
                        // metres from the title; cap the column like Roon does.
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.min(parent.width, 940)
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
                                    implicitWidth: 40
                                    implicitHeight: 40
                                    radius: 3
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
                                        color: root.textDim
                                        elide: Text.ElideRight
                                        visible: model.subtitle !== ""
                                    }
                                }

                                Label {
                                    text: "›"
                                    color: root.textOff
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

        // ─────────────────────────── Queue panel ───────────────────────────
        Rectangle {
            id: queuePanel
            anchors { right: parent.right; top: parent.top; bottom: transport.top }
            width: 336
            color: root.panelColor
            visible: root.queueOpen

            Rectangle {
                anchors { left: parent.left; top: parent.top; bottom: parent.bottom }
                width: 1
                color: root.lineColor
            }

            ColumnLayout {
                anchors.fill: parent
                anchors.leftMargin: 1
                spacing: 0

                RowLayout {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 58
                    Layout.leftMargin: 16
                    Layout.rightMargin: 8

                    Label {
                        Layout.fillWidth: true
                        text: "Queue"
                        font.pixelSize: 15
                        font.bold: true
                    }
                    Label {
                        text: queueModel.count > 0 ? queueModel.count : ""
                        font.pixelSize: 12
                        color: root.textDim
                    }
                    ToolButton {
                        icon.source: "qrc:/icons/forward.svg"
                        icon.width: 18
                        icon.height: 18
                        icon.color: "#c9cad4"
                        ToolTip.visible: hovered
                        ToolTip.text: "Hide queue"
                        onClicked: root.queueOpen = false
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    height: 1
                    color: root.lineColor
                }

                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    ListView {
                        id: queueList
                        anchors.fill: parent
                        anchors.margins: 6
                        clip: true
                        model: queueModel
                        onDraggingChanged: if (dragging) queueAnim.stop()

                        ScrollBar.vertical: ScrollBar {}

                        delegate: ItemDelegate {
                            width: queueList.width
                            onClicked: app.playFromHere(model.queueItemId)

                            contentItem: RowLayout {
                                spacing: 10

                                Rectangle {
                                    implicitWidth: 38
                                    implicitHeight: 38
                                    radius: 3
                                    color: "#191920"

                                    Image {
                                        anchors.fill: parent
                                        source: thumb(model.imageKey, 76)
                                        fillMode: Image.PreserveAspectCrop
                                        asynchronous: true
                                        visible: model.imageKey !== ""
                                    }
                                    Rectangle {
                                        anchors.fill: parent
                                        color: "#000000"
                                        opacity: 0.45
                                        visible: index === 0
                                    }
                                    Image {
                                        anchors.centerIn: parent
                                        source: app.playState === "Playing"
                                                ? "qrc:/icons/pause.svg" : "qrc:/icons/play.svg"
                                        sourceSize.width: 16
                                        sourceSize.height: 16
                                        visible: index === 0
                                    }
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 0

                                    Label {
                                        Layout.fillWidth: true
                                        text: model.title
                                        font.pixelSize: 13
                                        elide: Text.ElideRight
                                        color: index === 0 ? root.Material.accent : "#e8e8ee"
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: model.subtitle
                                        font.pixelSize: 11
                                        color: root.textDim
                                        elide: Text.ElideRight
                                        visible: model.subtitle !== ""
                                    }
                                }

                                Label {
                                    text: fmt(model.length)
                                    color: root.textDim
                                    font.pixelSize: 11
                                    visible: model.length > 0
                                }
                            }
                        }
                    }

                    ColumnLayout {
                        anchors.centerIn: parent
                        width: parent.width - 48
                        spacing: 6
                        visible: queueModel.count === 0

                        Label {
                            Layout.alignment: Qt.AlignHCenter
                            text: "Queue is empty"
                            color: root.textDim
                        }
                        Label {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                            font.pixelSize: 11
                            color: root.textOff
                            text: "Roon's API allows jumping within the queue, "
                                  + "but not adding, removing or reordering."
                        }
                    }

                    NumberAnimation {
                        id: queueAnim
                        target: queueList
                        property: "contentY"
                        duration: 160
                        easing.type: Easing.OutCubic
                    }

                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.NoButton
                        onWheel: wheel => root.wheelScroll(queueList, queueAnim, wheel)
                    }
                }
            }
        }

        // ────────────────────────── Transport bar ──────────────────────────
        Rectangle {
            id: transport
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
            height: 92
            color: root.panelColor

            Rectangle {
                anchors { left: parent.left; right: parent.right; top: parent.top }
                height: 1
                color: root.lineColor
            }

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 14
                anchors.rightMargin: 14
                spacing: 16

                // Now playing: art, metadata, expand.
                // The left and right groups both fill and share the remainder
                // equally, which is what keeps the fixed-width transport group
                // optically centred in the window.
                RowLayout {
                    // Equal preferredWidth on both flanking groups is what
                    // makes them share the remainder evenly; left to their own
                    // size hints the right group claims most of it and shoves
                    // the transport controls off centre.
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    Layout.minimumWidth: 210
                    spacing: 12

                    Rectangle {
                        implicitWidth: 62
                        implicitHeight: 62
                        radius: 4
                        color: "#191920"

                        Image {
                            anchors.fill: parent
                            source: app.artUrl
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            visible: status === Image.Ready
                        }
                        Image {
                            anchors.centerIn: parent
                            source: "qrc:/icons/note.svg"
                            sourceSize.width: 22
                            sourceSize.height: 22
                            opacity: 0.16
                            visible: app.artUrl.toString() === ""
                        }
                        Rectangle {
                            anchors.fill: parent
                            radius: 4
                            color: "#000000"
                            opacity: artHover.hovered ? 0.45 : 0
                            Behavior on opacity { NumberAnimation { duration: 120 } }
                        }
                        Image {
                            anchors.centerIn: parent
                            source: "qrc:/icons/chevron_up.svg"
                            sourceSize.width: 22
                            sourceSize.height: 22
                            opacity: artHover.hovered ? 0.95 : 0
                            Behavior on opacity { NumberAnimation { duration: 120 } }
                        }
                        HoverHandler {
                            id: artHover
                            cursorShape: Qt.PointingHandCursor
                        }
                        TapHandler { onTapped: root.npOpen = true }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.maximumWidth: 260
                        spacing: 1

                        Label {
                            Layout.fillWidth: true
                            text: app.title
                            font.pixelSize: 14
                            font.bold: true
                            elide: Text.ElideRight
                        }
                        Label {
                            Layout.fillWidth: true
                            text: app.artist
                            font.pixelSize: 12
                            color: "#b9bac4"
                            elide: Text.ElideRight
                        }
                        Label {
                            Layout.fillWidth: true
                            text: app.album
                            font.pixelSize: 11
                            color: root.textDim
                            elide: Text.ElideRight
                            visible: text !== ""
                        }
                    }
                    Item { Layout.fillWidth: true }
                }

                // Transport + seek.
                // fillWidth must be set false explicitly: a layout nested in
                // another layout defaults to true, which stretches this group
                // and drags the play button off centre.
                ColumnLayout {
                    Layout.fillWidth: false
                    Layout.preferredWidth: 560
                    spacing: 0

                    RowLayout {
                        Layout.alignment: Qt.AlignHCenter
                        spacing: 6

                        ToolButton {
                            icon.source: "qrc:/icons/shuffle.svg"
                            icon.width: 17
                            icon.height: 17
                            icon.color: app.shuffle ? root.Material.accent : "#6a6b75"
                            ToolTip.visible: hovered
                            ToolTip.text: app.shuffle ? "Shuffle on" : "Shuffle off"
                            onClicked: app.toggleShuffle()
                        }

                        ToolButton {
                            icon.source: "qrc:/icons/prev.svg"
                            icon.width: 22
                            icon.height: 22
                            icon.color: enabled ? "#c9cad4" : root.textOff
                            enabled: app.canPrevious
                            onClicked: app.previous()
                        }

                        RoundButton {
                            implicitWidth: 44
                            implicitHeight: 44
                            icon.source: app.playState === "Playing" ? "qrc:/icons/pause.svg"
                                                                     : "qrc:/icons/play.svg"
                            icon.width: 21
                            icon.height: 21
                            icon.color: "#101014"
                            Material.background: root.Material.accent
                            onClicked: app.playPause()
                        }

                        ToolButton {
                            icon.source: "qrc:/icons/next.svg"
                            icon.width: 22
                            icon.height: 22
                            icon.color: enabled ? "#c9cad4" : root.textOff
                            enabled: app.canNext
                            onClicked: app.next()
                        }

                        ToolButton {
                            icon.source: app.loopMode === "loop_one"
                                         ? "qrc:/icons/repeat_one.svg" : "qrc:/icons/repeat.svg"
                            icon.width: 17
                            icon.height: 17
                            icon.color: app.loopMode !== "disabled" ? root.Material.accent : "#6a6b75"
                            ToolTip.visible: hovered
                            ToolTip.text: app.loopMode === "loop" ? "Repeat queue"
                                        : app.loopMode === "loop_one" ? "Repeat track" : "Repeat off"
                            onClicked: app.cycleLoop()
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            text: fmt(seek.pressed ? seek.value : app.seekPosition)
                            font.pixelSize: 11
                            color: root.textDim
                        }

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

                        Label {
                            text: fmt(app.trackLength)
                            font.pixelSize: 11
                            color: root.textDim
                        }
                    }
                }

                // Volume, queue toggle, zone
                RowLayout {
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    Layout.minimumWidth: 340
                    spacing: 4

                    Item { Layout.fillWidth: true }

                    Image {
                        source: "qrc:/icons/volume.svg"
                        sourceSize.width: 17
                        sourceSize.height: 17
                        opacity: app.hasVolume ? 0.55 : 0.2
                        visible: app.hasVolume
                    }
                    Slider {
                        id: volume
                        Layout.preferredWidth: 96
                        from: app.volumeMin
                        to: app.volumeMax
                        visible: app.hasVolume
                        onMoved: app.changeVolume(value)

                        Binding on value {
                            when: !volume.pressed
                            value: app.volume
                        }
                    }

                    ToolButton {
                        icon.source: "qrc:/icons/queue.svg"
                        icon.width: 19
                        icon.height: 19
                        icon.color: root.queueOpen ? root.Material.accent : "#c9cad4"
                        ToolTip.visible: hovered
                        ToolTip.text: root.queueOpen ? "Hide queue (Ctrl+Q)" : "Show queue (Ctrl+Q)"
                        onClicked: root.queueOpen = !root.queueOpen
                    }

                    ToolButton {
                        icon.source: "qrc:/icons/link.svg"
                        icon.width: 18
                        icon.height: 18
                        icon.color: "#c9cad4"
                        ToolTip.visible: hovered
                        ToolTip.text: "Group zones"
                        onClicked: app.requestGroupInfo()
                    }

                    // Zone picker, in Roon's bottom-right corner position
                    ItemDelegate {
                        id: zoneButton
                        implicitHeight: 44
                        // Size from the delegate's own padding, not a guess,
                        // or the zone name elides to "DX3 P…".
                        implicitWidth: Math.min(260, zoneRow.implicitWidth
                                                + leftPadding + rightPadding)
                        onClicked: zoneMenu.open()

                        contentItem: RowLayout {
                            id: zoneRow
                            spacing: 8

                            Image {
                                source: "qrc:/icons/speaker.svg"
                                sourceSize.width: 17
                                sourceSize.height: 17
                                opacity: 0.65
                            }
                            Label {
                                Layout.maximumWidth: 200
                                text: app.zoneIndex >= 0 && app.zoneIndex < app.zoneList.length
                                      ? app.zoneList[app.zoneIndex] : "No zone"
                                font.pixelSize: 12
                                elide: Text.ElideRight
                            }
                        }

                        Menu {
                            id: zoneMenu
                            y: -height - 4

                            Repeater {
                                model: app.zoneList

                                MenuItem {
                                    required property int index
                                    required property string modelData
                                    text: modelData
                                    checkable: true
                                    checked: index === app.zoneIndex
                                    onTriggered: app.selectZone(index)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ══════════════════ Full-screen now playing (from the bar) ═════════════
    Rectangle {
        id: npView
        anchors.fill: parent
        color: "#0f0f13"
        visible: opacity > 0
        opacity: root.npOpen ? 1 : 0

        Behavior on opacity { NumberAnimation { duration: 160 } }

        Image {
            id: npBackdropSource
            anchors.fill: parent
            source: app.artUrl
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            visible: false
        }
        MultiEffect {
            anchors.fill: parent
            source: npBackdropSource
            blurEnabled: true
            blur: 1.0
            blurMax: 64
            opacity: 0.30
        }
        Rectangle {
            anchors.fill: parent
            color: "#0f0f13"
            opacity: 0.45
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 12

            RowLayout {
                Layout.fillWidth: true
                Layout.margins: 12
                Layout.bottomMargin: 0

                ToolButton {
                    icon.source: "qrc:/icons/chevron_down.svg"
                    icon.width: 22
                    icon.height: 22
                    icon.color: "#c9cad4"
                    ToolTip.visible: hovered
                    ToolTip.text: "Back to library (Esc)"
                    onClicked: root.npOpen = false
                }
                Item { Layout.fillWidth: true }
                Label {
                    text: app.zoneIndex >= 0 && app.zoneIndex < app.zoneList.length
                          ? app.zoneList[app.zoneIndex] : ""
                    font.pixelSize: 12
                    color: root.textDim
                }
            }

            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.leftMargin: 24
                Layout.rightMargin: 24

                Rectangle {
                    id: npArt
                    anchors.centerIn: parent
                    width: Math.min(parent.width, parent.height)
                    height: width
                    radius: 8
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
                    Image {
                        anchors.centerIn: parent
                        source: "qrc:/icons/note.svg"
                        sourceSize.width: npArt.width / 4
                        sourceSize.height: npArt.width / 4
                        opacity: 0.12
                        visible: app.artUrl.toString() === ""
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 32
                Layout.rightMargin: 32
                spacing: 2

                Label {
                    Layout.fillWidth: true
                    text: app.title
                    font.pixelSize: 24
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
                    color: root.textDim
                    elide: Text.ElideRight
                    horizontalAlignment: Text.AlignHCenter
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 32
                Layout.rightMargin: 32
                spacing: 10

                Label {
                    text: fmt(npSeek.pressed ? npSeek.value : app.seekPosition)
                    font.pixelSize: 12
                    color: root.textDim
                }
                Slider {
                    id: npSeek
                    Layout.fillWidth: true
                    from: 0
                    to: Math.max(1, app.trackLength)
                    enabled: app.trackLength > 0
                    onPressedChanged: if (!pressed) app.seekTo(value)

                    Binding on value {
                        when: !npSeek.pressed
                        value: app.seekPosition
                    }
                }
                Label {
                    text: fmt(app.trackLength)
                    font.pixelSize: 12
                    color: root.textDim
                }
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                Layout.bottomMargin: 24
                spacing: 20

                ToolButton {
                    icon.source: "qrc:/icons/shuffle.svg"
                    icon.width: 19
                    icon.height: 19
                    icon.color: app.shuffle ? root.Material.accent : "#6a6b75"
                    ToolTip.visible: hovered
                    ToolTip.text: app.shuffle ? "Shuffle on" : "Shuffle off"
                    onClicked: app.toggleShuffle()
                }
                RoundButton {
                    flat: true
                    icon.source: "qrc:/icons/prev.svg"
                    icon.width: 26
                    icon.height: 26
                    icon.color: enabled ? "#c9cad4" : root.textOff
                    implicitWidth: 54
                    implicitHeight: 54
                    enabled: app.canPrevious
                    onClicked: app.previous()
                }
                RoundButton {
                    implicitWidth: 72
                    implicitHeight: 72
                    icon.source: app.playState === "Playing" ? "qrc:/icons/pause.svg"
                                                             : "qrc:/icons/play.svg"
                    icon.width: 32
                    icon.height: 32
                    icon.color: "#101014"
                    Material.background: root.Material.accent
                    onClicked: app.playPause()
                }
                RoundButton {
                    flat: true
                    icon.source: "qrc:/icons/next.svg"
                    icon.width: 26
                    icon.height: 26
                    icon.color: enabled ? "#c9cad4" : root.textOff
                    implicitWidth: 54
                    implicitHeight: 54
                    enabled: app.canNext
                    onClicked: app.next()
                }
                ToolButton {
                    icon.source: "qrc:/icons/radio.svg"
                    icon.width: 19
                    icon.height: 19
                    icon.color: app.autoRadio ? root.Material.accent : "#6a6b75"
                    ToolTip.visible: hovered
                    ToolTip.text: app.autoRadio ? "Roon Radio on — keeps playing after the queue"
                                                : "Roon Radio off"
                    onClicked: app.toggleRadio()
                }
            }
        }
    }

    // ═══════════════════════════ Group zones dialog ════════════════════════
    Dialog {
        id: groupDialog
        modal: true
        title: "Group zones"
        anchors.centerIn: parent
        // Grow with the list, but never past the window. contentHeight comes
        // from the layout's implicit height, so header, footer and footnote
        // are accounted for by Dialog itself.
        width: Math.min(460, root.width - 48)
        contentHeight: Math.min(groupLayout.implicitHeight, root.height - 200)
        standardButtons: Dialog.Apply | Dialog.Cancel

        onApplied: {
            const ids = []
            for (let i = 0; i < groupModel.count; i++)
                if (groupModel.get(i).checked)
                    ids.push(groupModel.get(i).outputId)
            if (ids.length > 0)
                app.applyGrouping(JSON.stringify(ids))
            close()
        }

        ColumnLayout {
            id: groupLayout
            anchors.fill: parent
            spacing: 8

            ListView {
                id: groupList
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredHeight: contentHeight
                clip: true
                model: groupModel
                boundsBehavior: Flickable.StopAtBounds

                ScrollBar.vertical: ScrollBar {
                    policy: groupList.contentHeight > groupList.height
                            ? ScrollBar.AlwaysOn : ScrollBar.AsNeeded
                }

                delegate: CheckDelegate {
                    id: groupDelegate
                    width: groupList.width - (groupList.ScrollBar.vertical.visible
                                              ? groupList.ScrollBar.vertical.width : 0)
                    checked: model.checked
                    enabled: model.canGroup
                    onToggled: groupModel.setProperty(index, "checked", checked)

                    contentItem: ColumnLayout {
                        spacing: 0

                        Label {
                            Layout.fillWidth: true
                            text: model.name
                            elide: Text.ElideRight
                            color: groupDelegate.enabled ? "#e8e8ee" : "#5a5b65"
                        }
                        Label {
                            Layout.fillWidth: true
                            text: model.zoneName
                            font.pixelSize: 11
                            color: root.textDim
                            elide: Text.ElideRight
                            visible: model.zoneName !== model.name
                        }
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                visible: {
                    for (let i = 0; i < groupModel.count; i++)
                        if (!groupModel.get(i).canGroup)
                            return true
                    return false
                }
                wrapMode: Text.WordWrap
                font.pixelSize: 12
                color: root.textDim
                text: "Greyed-out outputs use a different audio protocol — "
                      + "Roon can only group matching outputs (e.g. RAAT with RAAT)."
            }
        }
    }

    // ═════════════════════════ Connection overlay ══════════════════════════
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

    // ═══════════════════════════════ Toast ═════════════════════════════════
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 112
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
