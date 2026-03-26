import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: page
    title: qsTr("Explorer")

    property string currentPath: ""
    property var entries: []
    property var filteredEntries: []
    property var selectedPaths: []
    property bool pendingSearchReset: true
    property string loadState: "loading"
    property string loadMessage: ""
    property int residencyFilter: 0

    readonly property string trimmedSearchText: searchField.text.trim()
    readonly property bool searchActive: trimmedSearchText.length > 0
    readonly property bool queryReady: shellBackend.remoteConfigured && shellBackend.daemonReachable
    readonly property bool canManageResidencyActions: queryReady && loadState !== "error" && loadState !== "unavailable"
    readonly property bool canOpenMountedPaths: shellBackend.mountState === "Running" && shellBackend.effectiveMountPath.length > 0
    readonly property color canvasColor: "#f3f6fb"
    readonly property color surfaceColor: "#ffffff"
    readonly property color mutedSurfaceColor: "#eef3f9"
    readonly property color lineColor: Qt.rgba(10 / 255, 28 / 255, 49 / 255, 0.08)
    readonly property color textMutedColor: "#5f6f82"

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

    function applyResult(result, clearSelection) {
        const resultState = String((result && result.state) || "error")
        const resultMessage = String((result && result.message) || "")
        const resultEntries = result && result.entries !== undefined ? result.entries : []

        page.entries = resultEntries
        if (resultState === "ok") {
            page.loadState = resultEntries.length > 0 ? "ready" : "empty"
            page.loadMessage = ""
        } else {
            page.loadState = resultState
            page.loadMessage = resultMessage
        }

        page.rebuildFilteredEntries()
        if (clearSelection) {
            page.clearSelection()
        } else {
            page.pruneSelectionTo(page.filteredEntries)
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

    function stateHint(entry) {
        const state = String(entry.state || "")
        if (state === "PinnedLocal") {
            return qsTr("Always keep a local copy")
        }
        if (state === "AvailableLocal") {
            return qsTr("Local copy already available")
        }
        if (state === "Syncing") {
            return qsTr("Transfer work in progress")
        }
        if (state === "Conflict") {
            return qsTr("Needs a manual retry")
        }
        if (state === "Error") {
            return qsTr("Last operation failed")
        }
        return qsTr("Download when first opened")
    }

    function stateColor(entry) {
        const state = String(entry.state || "")
        if (state === "PinnedLocal") {
            return "#147a51"
        }
        if (state === "AvailableLocal") {
            return "#215f9b"
        }
        if (state === "Syncing") {
            return "#2e75c8"
        }
        if (state === "Conflict" || state === "Error") {
            return "#b53b2d"
        }
        return "#7a8796"
    }

    function stateGroup(entry) {
        const state = String(entry.state || "")
        if (state === "OnlineOnly") {
            return 1
        }
        if (state === "PinnedLocal" || state === "AvailableLocal") {
            return 2
        }
        return 3
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

    function pruneSelectionTo(entries) {
        const valid = []
        for (let index = 0; index < page.selectedPaths.length; ++index) {
            const path = page.selectedPaths[index]
            for (let entryIndex = 0; entryIndex < entries.length; ++entryIndex) {
                if (entries[entryIndex].path === path) {
                    valid.push(path)
                    break
                }
            }
        }
        page.selectedPaths = valid
    }

    function filterMatches(entry) {
        if (page.residencyFilter === 0) {
            return true
        }
        return page.stateGroup(entry) === page.residencyFilter
    }

    function countForFilter(filterIndex) {
        let count = 0
        for (let index = 0; index < page.entries.length; ++index) {
            if (filterIndex === 0 || page.stateGroup(page.entries[index]) === filterIndex) {
                count += 1
            }
        }
        return count
    }

    function rebuildFilteredEntries() {
        const next = []
        for (let index = 0; index < page.entries.length; ++index) {
            const entry = page.entries[index]
            if (page.filterMatches(entry)) {
                next.push(entry)
            }
        }
        page.filteredEntries = next
        page.pruneSelectionTo(next)
    }

    function refresh(clearSelection) {
        if (!shellBackend.remoteConfigured) {
            page.entries = []
            page.filteredEntries = []
            page.loadState = "unconfigured"
            page.loadMessage = qsTr("Connect OneDrive and choose a visible folder before browsing files here.")
            if (clearSelection) {
                page.clearSelection()
            }
            return
        }

        const result = page.searchActive
                ? shellBackend.searchPathsResult(page.trimmedSearchText, 200)
                : shellBackend.listDirectoryResult(page.currentPath)
        page.applyResult(result, clearSelection)
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
        if (!page.canOpenMountedPaths) {
            return
        }
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

    function canOpenEntry(entry) {
        return entry.is_dir || page.canOpenMountedPaths
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
        if (page.canOpenMountedPaths) {
            shellBackend.openPath(entry.path)
        }
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
            return qsTr("%1 search result(s)").arg(page.filteredEntries.length)
        }
        return qsTr("%1 item(s) in %2").arg(page.filteredEntries.length).arg(page.currentPath.length > 0 ? page.currentPath : qsTr("root"))
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

        function onRemoteConfiguredChanged() {
            page.refresh(true)
        }

        function onDaemonReachableChanged() {
            page.refresh(true)
        }
    }

    Rectangle {
        anchors.fill: parent
        color: page.canvasColor
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing
            color: page.surfaceColor
            border.width: 1
            border.color: page.lineColor

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.mediumSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Kirigami.Heading {
                            text: qsTr("Explorer")
                            level: 1
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: page.textMutedColor
                            text: qsTr("Browse daemon-backed path state, separate online-only items from local ones, and run residency actions without dropping into the CLI.")
                        }
                    }

                    Button {
                        text: qsTr("Refresh")
                        icon.name: "view-refresh"
                        enabled: shellBackend.remoteConfigured
                        onClicked: page.requestRefresh(false)
                    }

                    Button {
                        text: qsTr("Start Filesystem")
                        icon.name: "folder-cloud"
                        visible: shellBackend.canMount
                        onClicked: shellBackend.mountRemote()
                    }
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    visible: page.loadState === "unconfigured"
                    type: Kirigami.MessageType.Information
                    showCloseButton: false
                    text: page.loadMessage
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    visible: page.loadState === "unavailable" || page.loadState === "error"
                    type: page.loadState === "unavailable" ? Kirigami.MessageType.Warning : Kirigami.MessageType.Error
                    showCloseButton: false
                    text: page.loadMessage
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    visible: page.queryReady && !page.canOpenMountedPaths
                    type: Kirigami.MessageType.Information
                    showCloseButton: false
                    text: qsTr("Explorer can still inspect daemon state and queue residency changes, but the visible folder is not mounted yet. Start the filesystem to open items from the file manager.")
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
                        enabled: page.canOpenMountedPaths
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

                    Rectangle {
                        radius: Kirigami.Units.largeSpacing
                        color: page.mutedSurfaceColor
                        border.width: 1
                        border.color: page.lineColor
                        Layout.fillWidth: true

                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: Kirigami.Units.smallSpacing
                            spacing: Kirigami.Units.smallSpacing

                            Button {
                                text: qsTr("All · %1").arg(page.countForFilter(0))
                                checkable: true
                                checked: page.residencyFilter === 0
                                onClicked: {
                                    page.residencyFilter = 0
                                    page.rebuildFilteredEntries()
                                }
                            }

                            Button {
                                text: qsTr("Online-only · %1").arg(page.countForFilter(1))
                                checkable: true
                                checked: page.residencyFilter === 1
                                onClicked: {
                                    page.residencyFilter = 1
                                    page.rebuildFilteredEntries()
                                }
                            }

                            Button {
                                text: qsTr("Local · %1").arg(page.countForFilter(2))
                                checkable: true
                                checked: page.residencyFilter === 2
                                onClicked: {
                                    page.residencyFilter = 2
                                    page.rebuildFilteredEntries()
                                }
                            }

                            Button {
                                text: qsTr("Attention · %1").arg(page.countForFilter(3))
                                checkable: true
                                checked: page.residencyFilter === 3
                                onClicked: {
                                    page.residencyFilter = 3
                                    page.rebuildFilteredEntries()
                                }
                            }
                        }
                    }

                    Flow {
                        spacing: Kirigami.Units.smallSpacing

                        Repeater {
                            model: [
                                { "label": qsTr("Online-only"), "color": "#7a8796" },
                                { "label": qsTr("Available offline"), "color": "#215f9b" },
                                { "label": qsTr("Kept on device"), "color": "#147a51" },
                                { "label": qsTr("Attention"), "color": "#b53b2d" }
                            ]

                            delegate: Rectangle {
                                required property var modelData
                                radius: 999
                                color: Qt.lighter(modelData.color, 1.8)
                                implicitWidth: legendLabel.implicitWidth + Kirigami.Units.mediumSpacing * 2
                                implicitHeight: legendLabel.implicitHeight + Kirigami.Units.smallSpacing * 2

                                Label {
                                    id: legendLabel
                                    anchors.centerIn: parent
                                    color: modelData.color
                                    text: modelData.label
                                    font.bold: true
                                }
                            }
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
                        enabled: page.selectedCount() > 0 && page.canManageResidencyActions
                        onClicked: page.runSelectionAction(shellBackend.keepLocalPaths)
                    }

                    Button {
                        text: qsTr("Make online-only")
                        icon.name: "folder-download"
                        enabled: page.selectedCount() > 0 && page.canManageResidencyActions
                        onClicked: page.runSelectionAction(shellBackend.makeOnlineOnlyPaths)
                    }

                    Button {
                        text: qsTr("Retry transfer")
                        icon.name: "view-refresh"
                        enabled: page.selectedCount() > 0 && page.canManageResidencyActions
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
                        color: page.textMutedColor
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Kirigami.Units.largeSpacing
            color: page.surfaceColor
            border.width: 1
            border.color: page.lineColor

            Loader {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                active: true
                sourceComponent: {
                    if (page.loadState === "loading") {
                        return loadingState
                    }
                    if (page.loadState === "unconfigured" || page.loadState === "unavailable" || page.loadState === "error") {
                        return messageState
                    }
                    if (page.loadState === "empty") {
                        return emptyState
                    }
                    return listState
                }
            }
        }
    }

    Component {
        id: loadingState

        ColumnLayout {
            anchors.centerIn: parent
            spacing: Kirigami.Units.mediumSpacing

            BusyIndicator {
                Layout.alignment: Qt.AlignHCenter
                running: true
            }

            Label {
                text: qsTr("Refreshing Explorer…")
                font.bold: true
            }
        }
    }

    Component {
        id: messageState

        ColumnLayout {
            anchors.centerIn: parent
            width: Math.min(parent.width, 560)
            spacing: Kirigami.Units.mediumSpacing

            Kirigami.Icon {
                Layout.alignment: Qt.AlignHCenter
                source: page.loadState === "error" ? "dialog-error" : "network-disconnect"
                implicitWidth: Kirigami.Units.iconSizes.huge
                implicitHeight: Kirigami.Units.iconSizes.huge
            }

            Kirigami.Heading {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                level: 2
                text: page.loadState === "unconfigured"
                      ? qsTr("Connect OneDrive first")
                      : page.loadState === "unavailable"
                        ? qsTr("Explorer is waiting for the daemon")
                        : qsTr("Explorer data could not be loaded")
            }

            Label {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                color: page.textMutedColor
                text: page.loadMessage
            }
        }
    }

    Component {
        id: emptyState

        ColumnLayout {
            anchors.centerIn: parent
            width: Math.min(parent.width, 520)
            spacing: Kirigami.Units.mediumSpacing

            Kirigami.Icon {
                Layout.alignment: Qt.AlignHCenter
                source: page.searchActive ? "edit-find" : "folder"
                implicitWidth: Kirigami.Units.iconSizes.huge
                implicitHeight: Kirigami.Units.iconSizes.huge
            }

            Kirigami.Heading {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                level: 2
                text: page.searchActive
                      ? qsTr("No files match this search")
                      : qsTr("No items are visible here yet")
            }

            Label {
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                color: page.textMutedColor
                text: page.searchActive
                      ? qsTr("Try a broader search term or switch the residency filter back to All.")
                      : qsTr("If you expected online-only files here, run a refresh or review the daemon status above instead of assuming the folder is really empty.")
            }
        }
    }

    Component {
        id: listState

        ListView {
            clip: true
            spacing: Kirigami.Units.smallSpacing
            model: page.filteredEntries

            delegate: Rectangle {
                required property var modelData
                readonly property var entry: modelData
                readonly property string entryName: String(entry.path || "").split("/").slice(-1)[0]

                width: ListView.view.width
                radius: Kirigami.Units.largeSpacing
                color: page.isSelected(entry.path) ? "#ecf3ff" : "#ffffff"
                border.width: 1
                border.color: page.isSelected(entry.path) ? "#b6cae5" : page.lineColor
                implicitHeight: rowLayout.implicitHeight + Kirigami.Units.largeSpacing

                Rectangle {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: 6
                    radius: Kirigami.Units.largeSpacing
                    color: page.stateColor(entry)
                }

                RowLayout {
                    id: rowLayout
                    anchors.fill: parent
                    anchors.leftMargin: Kirigami.Units.largeSpacing
                    anchors.rightMargin: Kirigami.Units.mediumSpacing
                    anchors.topMargin: Kirigami.Units.mediumSpacing
                    anchors.bottomMargin: Kirigami.Units.mediumSpacing
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
                            flat: true
                            text: entryName
                            font.bold: true
                            enabled: page.canOpenEntry(entry)
                            onClicked: page.showEntry(entry)
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: page.stateHint(entry)
                            color: page.stateColor(entry)
                            font.bold: true
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: page.textMutedColor
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
                        enabled: page.canManageResidencyActions
                        onClicked: page.runQuickAction(entry)
                    }

                    Button {
                        text: entry.is_dir ? qsTr("Browse") : qsTr("Open")
                        icon.name: entry.is_dir ? "go-next" : "document-open"
                        enabled: page.canOpenEntry(entry)
                        onClicked: page.showEntry(entry)
                    }
                }
            }
        }
    }
}
