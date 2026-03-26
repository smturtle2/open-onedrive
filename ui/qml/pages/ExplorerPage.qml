import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: page
    title: qsTr("Explorer")

    property string currentPath: ""
    property var entries: []
    property var selectedPaths: []
    property bool pendingSearchReset: true

    readonly property string trimmedSearchText: searchField.text.trim()
    readonly property bool searchActive: trimmedSearchText.length > 0
    readonly property bool readyForBrowsing: shellBackend.remoteConfigured && shellBackend.daemonReachable

    function normalizePath(path) {
        let normalized = String(path || "")
        while (normalized.startsWith("/")) {
            normalized = normalized.slice(1)
        }
        while (normalized.endsWith("/") && normalized.length > 0) {
            normalized = normalized.slice(0, normalized.length - 1)
        }
        return normalized
    }

    function parseEntries(payload) {
        try {
            const parsed = JSON.parse(payload)
            return Array.isArray(parsed) ? parsed : []
        } catch (error) {
            return []
        }
    }

    function formatTimestamp(unixSeconds) {
        if (!unixSeconds || unixSeconds <= 0) {
            return qsTr("Not synced yet")
        }
        return Qt.formatDateTime(new Date(unixSeconds * 1000), "yyyy-MM-dd hh:mm")
    }

    function formatBytes(bytes) {
        const size = Number(bytes || 0)
        if (size <= 0) {
            return qsTr("0 B")
        }
        const units = ["B", "KB", "MB", "GB", "TB"]
        let value = size
        let unitIndex = 0
        while (value >= 1024 && unitIndex < units.length - 1) {
            value /= 1024
            unitIndex += 1
        }
        const decimals = value >= 10 || unitIndex === 0 ? 0 : 1
        return Number(value).toFixed(decimals) + " " + units[unitIndex]
    }

    function stateLabel(entry) {
        const state = String(entry.state || "")
        if (state === "PinnedLocal") {
            return qsTr("Kept on device")
        }
        if (state === "AvailableLocal") {
            return qsTr("Available offline")
        }
        if (state === "Syncing") {
            return qsTr("Syncing")
        }
        if (state === "Conflict") {
            return qsTr("Conflict")
        }
        if (state === "Error") {
            return qsTr("Error")
        }
        return qsTr("Online-only")
    }

    function stateColor(entry) {
        const state = String(entry.state || "")
        if (state === "PinnedLocal") {
            return "#1f7a4d"
        }
        if (state === "AvailableLocal") {
            return "#295c8a"
        }
        if (state === "Syncing") {
            return "#3c73d4"
        }
        if (state === "Conflict" || state === "Error") {
            return "#b3261e"
        }
        return "#6a7783"
    }

    function detailText(entry) {
        const notes = []
        notes.push(entry.is_dir ? qsTr("Folder") : page.formatBytes(entry.size_bytes))
        if (entry.dirty) {
            notes.push(qsTr("Pending upload"))
        }
        if (String(entry.error || "").length > 0) {
            notes.push(entry.error)
        } else if (String(entry.conflict_reason || "").length > 0) {
            notes.push(entry.conflict_reason)
        }
        notes.push(qsTr("Last sync %1").arg(page.formatTimestamp(entry.last_sync_at)))
        return notes.join("  ·  ")
    }

    function breadcrumbParts() {
        const parts = [{
            "label": qsTr("Root"),
            "path": ""
        }]
        const segments = page.currentPath.length > 0 ? page.currentPath.split("/") : []
        let assembled = ""
        for (let index = 0; index < segments.length; ++index) {
            assembled = assembled.length > 0 ? assembled + "/" + segments[index] : segments[index]
            parts.push({
                "label": segments[index],
                "path": assembled
            })
        }
        return parts
    }

    function selectedCount() {
        return page.selectedPaths.length
    }

    function isSelected(path) {
        return page.selectedPaths.indexOf(path) >= 0
    }

    function toggleSelection(path) {
        const next = page.selectedPaths.slice(0)
        const existingIndex = next.indexOf(path)
        if (existingIndex >= 0) {
            next.splice(existingIndex, 1)
        } else {
            next.push(path)
        }
        page.selectedPaths = next
    }

    function clearSelection() {
        page.selectedPaths = []
    }

    function pruneSelection() {
        const valid = []
        for (let index = 0; index < page.selectedPaths.length; ++index) {
            const path = page.selectedPaths[index]
            for (let entryIndex = 0; entryIndex < page.entries.length; ++entryIndex) {
                if (page.entries[entryIndex].path === path) {
                    valid.push(path)
                    break
                }
            }
        }
        page.selectedPaths = valid
    }

    function refresh(clearSelection) {
        const payload = page.searchActive
                ? shellBackend.searchPathsJson(page.trimmedSearchText, 200)
                : shellBackend.listDirectoryJson(page.currentPath)
        page.entries = page.parseEntries(payload)
        if (clearSelection) {
            page.clearSelection()
        } else {
            page.pruneSelection()
        }
    }

    function requestRefresh(clearSelection) {
        if (page.searchActive) {
            page.pendingSearchReset = clearSelection
            searchDebounce.restart()
            return
        }
        page.refresh(clearSelection)
    }

    function openCurrentFolder() {
        if (page.currentPath.length > 0) {
            shellBackend.openPath(page.currentPath)
            return
        }
        shellBackend.openPath(shellBackend.effectiveMountPath)
    }

    function goUp() {
        if (page.currentPath.length === 0) {
            return
        }
        const segments = page.currentPath.split("/")
        segments.pop()
        page.currentPath = segments.join("/")
        page.refresh(true)
    }

    function showEntry(entry) {
        if (entry.is_dir) {
            page.currentPath = page.normalizePath(entry.path)
            if (page.searchActive) {
                searchField.text = ""
            }
            page.refresh(true)
            return
        }
        shellBackend.openPath(entry.path)
    }

    function runSelectionAction(callback) {
        if (page.selectedPaths.length === 0) {
            return
        }
        callback(page.selectedPaths)
        page.refresh(false)
    }

    function quickActionText(entry) {
        const state = String(entry.state || "")
        if (state === "Conflict" || state === "Error") {
            return qsTr("Retry")
        }
        if (state === "OnlineOnly") {
            return qsTr("Keep")
        }
        return qsTr("Online-only")
    }

    function quickActionIcon(entry) {
        const state = String(entry.state || "")
        if (state === "Conflict" || state === "Error") {
            return "view-refresh"
        }
        if (state === "OnlineOnly") {
            return "emblem-favorite"
        }
        return "folder-download"
    }

    function runQuickAction(entry) {
        const state = String(entry.state || "")
        if (state === "Conflict" || state === "Error") {
            shellBackend.retryTransferPath(entry.path)
        } else if (state === "OnlineOnly") {
            shellBackend.keepLocalPath(entry.path)
        } else {
            shellBackend.makeOnlineOnlyPath(entry.path)
        }
        page.refresh(false)
    }

    function listCountLabel() {
        if (page.searchActive) {
            return qsTr("%1 search result(s)").arg(page.entries.length)
        }
        return qsTr("%1 item(s) in %2").arg(page.entries.length).arg(page.currentPath.length > 0 ? page.currentPath : qsTr("root"))
    }

    Timer {
        id: searchDebounce
        interval: 280
        repeat: false
        onTriggered: page.refresh(page.pendingSearchReset)
    }

    Component.onCompleted: page.refresh(true)

    Connections {
        target: shellBackend

        function onPathStatesChanged() {
            page.refresh(false)
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing
            color: "#f7fafc"
            border.width: 1
            border.color: Qt.rgba(7 / 255, 31 / 255, 52 / 255, 0.08)

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.mediumSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.mediumSpacing

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Kirigami.Heading {
                            text: qsTr("Browse the visible OneDrive folder")
                            level: 1
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Kirigami.Theme.neutralTextColor
                            text: qsTr("Search paths, open files on demand, and switch selected items between offline and online-only without typing commands.")
                        }
                    }

                    Button {
                        text: qsTr("Refresh")
                        icon.name: "view-refresh"
                        enabled: shellBackend.remoteConfigured
                        onClicked: page.requestRefresh(false)
                    }
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    visible: !shellBackend.remoteConfigured || !shellBackend.daemonReachable
                    type: !shellBackend.remoteConfigured ? Kirigami.MessageType.Information : Kirigami.MessageType.Warning
                    showCloseButton: false
                    text: !shellBackend.remoteConfigured
                          ? qsTr("Connect OneDrive and choose a visible folder before browsing files here.")
                          : qsTr("The background service is offline. Explorer will refresh again when file-state updates come back.")
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    TextField {
                        id: searchField
                        Layout.fillWidth: true
                        placeholderText: qsTr("Search files and folders")

                        onTextChanged: {
                            page.pendingSearchReset = true
                            searchDebounce.restart()
                        }
                    }

                    Button {
                        text: qsTr("Clear")
                        visible: page.searchActive
                        onClicked: {
                            searchField.text = ""
                            page.refresh(true)
                        }
                    }

                    Button {
                        text: qsTr("Up")
                        icon.name: "go-up"
                        enabled: !page.searchActive && page.currentPath.length > 0
                        onClicked: page.goUp()
                    }

                    Button {
                        text: qsTr("Open Folder")
                        icon.name: "document-open-folder"
                        enabled: shellBackend.effectiveMountPath.length > 0
                        onClicked: page.openCurrentFolder()
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Repeater {
                        model: page.breadcrumbParts()

                        delegate: Button {
                            required property var modelData
                            text: modelData.label
                            enabled: !page.searchActive
                            highlighted: modelData.path === page.currentPath

                            onClicked: {
                                page.currentPath = modelData.path
                                page.refresh(true)
                            }
                        }
                    }

                    Rectangle {
                        visible: page.searchActive
                        radius: 999
                        color: "#dce9ff"
                        implicitWidth: searchBadge.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: searchBadge.implicitHeight + Kirigami.Units.smallSpacing * 2

                        Label {
                            id: searchBadge
                            anchors.centerIn: parent
                            color: "#244a86"
                            font.bold: true
                            text: qsTr("Searching: %1").arg(page.trimmedSearchText)
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Button {
                        text: qsTr("Keep on device")
                        icon.name: "emblem-favorite"
                        highlighted: true
                        enabled: page.selectedCount() > 0 && page.readyForBrowsing
                        onClicked: page.runSelectionAction(shellBackend.keepLocalPaths)
                    }

                    Button {
                        text: qsTr("Make online-only")
                        icon.name: "folder-download"
                        enabled: page.selectedCount() > 0 && page.readyForBrowsing
                        onClicked: page.runSelectionAction(shellBackend.makeOnlineOnlyPaths)
                    }

                    Button {
                        text: qsTr("Retry transfer")
                        icon.name: "view-refresh"
                        enabled: page.selectedCount() > 0 && page.readyForBrowsing
                        onClicked: page.runSelectionAction(shellBackend.retryTransferPaths)
                    }

                    Button {
                        text: qsTr("Clear selection")
                        enabled: page.selectedCount() > 0
                        onClicked: page.clearSelection()
                    }

                    Item {
                        Layout.fillWidth: true
                    }

                    Label {
                        text: page.selectedCount() > 0
                              ? qsTr("%1 selected").arg(page.selectedCount())
                              : page.listCountLabel()
                        color: Kirigami.Theme.neutralTextColor
                    }
                }
            }
        }

        Label {
            Layout.fillWidth: true
            visible: page.readyForBrowsing && page.entries.length === 0
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.neutralTextColor
            text: page.searchActive
                  ? qsTr("No files or folders match the current search.")
                  : qsTr("This folder is empty.")
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: Kirigami.Units.smallSpacing
            model: page.entries

            delegate: Rectangle {
                required property var modelData
                readonly property var entry: modelData

                width: ListView.view.width
                radius: Kirigami.Units.largeSpacing
                color: page.isSelected(entry.path) ? "#e9f0fb" : "#ffffff"
                border.width: 1
                border.color: page.isSelected(entry.path)
                              ? "#b9cee8"
                              : Qt.rgba(7 / 255, 31 / 255, 52 / 255, 0.08)

                implicitHeight: rowLayout.implicitHeight + Kirigami.Units.largeSpacing

                RowLayout {
                    id: rowLayout
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.mediumSpacing
                    spacing: Kirigami.Units.mediumSpacing

                    CheckBox {
                        checked: page.isSelected(entry.path)
                        onClicked: page.toggleSelection(entry.path)
                    }

                    Kirigami.Icon {
                        source: entry.is_dir ? "folder" : "text-x-generic"
                        implicitWidth: Kirigami.Units.iconSizes.medium
                        implicitHeight: Kirigami.Units.iconSizes.medium
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2

                        Button {
                            Layout.alignment: Qt.AlignLeft
                            text: String(entry.path || "").split("/").slice(-1)[0]
                            flat: true
                            font.bold: true
                            onClicked: page.showEntry(entry)
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Kirigami.Theme.neutralTextColor
                            text: page.detailText(entry)
                        }

                        Label {
                            Layout.fillWidth: true
                            visible: page.searchActive
                            wrapMode: Text.WordWrap
                            color: Kirigami.Theme.disabledTextColor
                            text: entry.path
                        }
                    }

                    Rectangle {
                        radius: 999
                        color: page.stateColor(entry)
                        implicitHeight: stateLabel.implicitHeight + Kirigami.Units.smallSpacing * 2
                        implicitWidth: stateLabel.implicitWidth + Kirigami.Units.mediumSpacing * 2

                        Label {
                            id: stateLabel
                            anchors.centerIn: parent
                            text: page.stateLabel(entry)
                            color: "white"
                            font.bold: true
                        }
                    }

                    Button {
                        text: page.quickActionText(entry)
                        icon.name: page.quickActionIcon(entry)
                        enabled: page.readyForBrowsing
                        onClicked: page.runQuickAction(entry)
                    }

                    Button {
                        text: entry.is_dir ? qsTr("Browse") : qsTr("Open")
                        icon.name: entry.is_dir ? "go-next" : "document-open"
                        onClicked: page.showEntry(entry)
                    }
                }
            }
        }
    }
}
