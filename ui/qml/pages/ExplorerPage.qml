import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: page
    title: qsTr("Files")

    property string currentPath: ""
    property var entries: []
    property var filteredEntries: []
    property var selectedPaths: []
    property bool pendingSearchReset: true
    property string loadState: "loading"
    property string loadMessage: ""
    property int residencyFilter: 0
    property string contextPath: ""

    readonly property string trimmedSearchText: searchField.text.trim()
    readonly property bool searchActive: trimmedSearchText.length > 0
    readonly property bool queryReady: shellBackend.remoteConfigured && shellBackend.daemonReachable
    readonly property bool canManageResidencyActions: queryReady && loadState !== "error" && loadState !== "unavailable"
    readonly property bool canOpenMountedPaths: shellBackend.mountState === "Running" && shellBackend.effectiveMountPath.length > 0
    readonly property color canvasColor: "#f4f7fa"
    readonly property color surfaceColor: "#ffffff"
    readonly property color panelColor: "#f5f8fb"
    readonly property color selectedRowColor: "#edf4ff"
    readonly property color lineColor: Qt.rgba(11 / 255, 28 / 255, 45 / 255, 0.09)
    readonly property color textMutedColor: "#627284"
    readonly property var inspectorEntry: page.selectedCount() === 1 ? page.entryForPath(page.selectedPaths[0]) : null

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

    function entryName(entry) {
        const path = String((entry && entry.path) || "")
        const parts = path.split("/")
        return parts.length > 0 ? parts[parts.length - 1] : path
    }

    function entryForPath(path) {
        for (let index = 0; index < page.entries.length; ++index) {
            if (page.entries[index].path === path) {
                return page.entries[index]
            }
        }
        return null
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
        const state = String((entry && entry.state) || "")
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
        const state = String((entry && entry.state) || "")
        if (state === "PinnedLocal") {
            return qsTr("Always keep a local copy")
        }
        if (state === "AvailableLocal") {
            return qsTr("Local bytes are already available")
        }
        if (state === "Syncing") {
            return qsTr("Transfer work in progress")
        }
        if (state === "Conflict") {
            return qsTr("Needs a retry")
        }
        if (state === "Error") {
            return qsTr("Last operation failed")
        }
        return qsTr("Visible now. Downloads when opened or pinned")
    }

    function stateColor(entry) {
        const state = String((entry && entry.state) || "")
        if (state === "PinnedLocal") {
            return "#147a51"
        }
        if (state === "AvailableLocal") {
            return "#215f9b"
        }
        if (state === "Syncing") {
            return "#d38a1b"
        }
        if (state === "Conflict" || state === "Error") {
            return "#b53b2d"
        }
        return "#3d77d9"
    }

    function stateIcon(entry) {
        const state = String((entry && entry.state) || "")
        if (state === "PinnedLocal") {
            return "emblem-favorite"
        }
        if (state === "AvailableLocal") {
            return "emblem-checked"
        }
        if (state === "Syncing") {
            return "emblem-synchronizing"
        }
        if (state === "Conflict" || state === "Error") {
            return "emblem-important"
        }
        return "folder-cloud"
    }

    function stateGroup(entry) {
        const state = String((entry && entry.state) || "")
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

    function selectOnly(path) {
        page.selectedPaths = path.length > 0 ? [path] : []
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
        const state = String((entry && entry.state) || "")
        if (state === "Conflict" || state === "Error") {
            return qsTr("Retry")
        }
        if (state === "OnlineOnly") {
            return qsTr("Keep on device")
        }
        return qsTr("Make online-only")
    }

    function quickActionIcon(entry) {
        const state = String((entry && entry.state) || "")
        if (state === "Conflict" || state === "Error") {
            return "view-refresh"
        }
        if (state === "OnlineOnly") {
            return "emblem-favorite"
        }
        return "folder-download"
    }

    function runQuickAction(entry) {
        const state = String((entry && entry.state) || "")
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

    function selectionSummaryLabel() {
        if (page.selectedCount() === 0) {
            return qsTr("No selection")
        }
        if (page.selectedCount() === 1 && page.inspectorEntry) {
            return page.entryName(page.inspectorEntry)
        }
        return qsTr("%1 items selected").arg(page.selectedCount())
    }

    function selectionSummaryBody() {
        if (page.selectedCount() === 0) {
            return qsTr("Select a file or folder to inspect residency, sync state, and quick actions.")
        }
        if (page.selectedCount() === 1 && page.inspectorEntry) {
            return page.detailText(page.inspectorEntry)
        }
        return qsTr("Bulk actions use the same queued daemon workflow as file-manager actions.")
    }

    function currentContextPaths() {
        if (page.contextPath.length === 0) {
            return []
        }
        if (page.selectedPaths.indexOf(page.contextPath) >= 0 && page.selectedPaths.length > 1) {
            return page.selectedPaths
        }
        return [page.contextPath]
    }

    function openContextMenu(entry, item, localX, localY) {
        page.contextPath = entry.path
        if (!page.isSelected(entry.path)) {
            page.selectOnly(entry.path)
        }
        const point = item.mapToItem(page, localX, localY)
        entryContextMenu.x = point.x
        entryContextMenu.y = point.y
        entryContextMenu.open()
    }

    Timer {
        id: searchDebounce
        interval: 280
        repeat: false
        onTriggered: page.refresh(page.pendingSearchReset)
    }

    Menu {
        id: entryContextMenu

        readonly property var contextEntry: page.entryForPath(page.contextPath)
        readonly property string contextState: contextEntry ? String(contextEntry.state || "") : ""
        readonly property bool canRetryContext: contextState === "Conflict" || contextState === "Error"
        readonly property bool canKeepContext: contextState === "OnlineOnly"
        readonly property bool canMakeOnlineOnlyContext: contextEntry && !canRetryContext && contextState !== "OnlineOnly"

        MenuItem {
            text: entryContextMenu.contextEntry && entryContextMenu.contextEntry.is_dir ? qsTr("Browse folder") : qsTr("Open")
            icon.name: entryContextMenu.contextEntry && entryContextMenu.contextEntry.is_dir ? "go-next" : "document-open"
            enabled: entryContextMenu.contextEntry && page.canOpenEntry(entryContextMenu.contextEntry)
            onTriggered: page.showEntry(entryContextMenu.contextEntry)
        }

        MenuSeparator { }

        MenuItem {
            text: qsTr("Keep on device")
            icon.name: "emblem-favorite"
            enabled: page.canManageResidencyActions && entryContextMenu.canKeepContext
            onTriggered: {
                const paths = page.currentContextPaths()
                if (paths.length > 0) {
                    shellBackend.keepLocalPaths(paths)
                    page.refresh(false)
                }
            }
        }

        MenuItem {
            text: qsTr("Free up space")
            icon.name: "folder-download"
            enabled: page.canManageResidencyActions && entryContextMenu.canMakeOnlineOnlyContext
            onTriggered: {
                const paths = page.currentContextPaths()
                if (paths.length > 0) {
                    shellBackend.makeOnlineOnlyPaths(paths)
                    page.refresh(false)
                }
            }
        }

        MenuItem {
            text: qsTr("Retry transfer")
            icon.name: "view-refresh"
            enabled: page.canManageResidencyActions && entryContextMenu.canRetryContext
            onTriggered: {
                const paths = page.currentContextPaths()
                if (paths.length > 0) {
                    shellBackend.retryTransferPaths(paths)
                    page.refresh(false)
                }
            }
        }
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
        spacing: Kirigami.Units.largeSpacing

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Heading {
                    text: qsTr("Files")
                    level: 1
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: page.textMutedColor
                    text: qsTr("Online-only files stay visible here. Keep items on this device or free up space from the same list.")
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
            text: qsTr("The file list can still inspect daemon state and queue residency actions, but the visible folder is not mounted yet.")
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.loadState === "ready" && page.countForFilter(1) > 0
            type: Kirigami.MessageType.Information
            showCloseButton: false
            text: qsTr("%1 online-only item(s) are visible in this folder right now.").arg(page.countForFilter(1))
        }

        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing
            color: page.surfaceColor
            border.width: 1
            border.color: page.lineColor

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.mediumSpacing
                spacing: Kirigami.Units.mediumSpacing

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
                            flat: modelData.path !== page.currentPath
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
                        color: "#e1ecff"
                        implicitWidth: searchBadge.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: searchBadge.implicitHeight + Kirigami.Units.smallSpacing * 2

                        Label {
                            id: searchBadge
                            anchors.centerIn: parent
                            color: "#244a86"
                            font.bold: true
                            text: qsTr("Searching %1").arg(page.trimmedSearchText)
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Repeater {
                        model: [
                            { "index": 0, "label": qsTr("All"), "count": page.countForFilter(0) },
                            { "index": 1, "label": qsTr("Online-only"), "count": page.countForFilter(1) },
                            { "index": 2, "label": qsTr("Local"), "count": page.countForFilter(2) },
                            { "index": 3, "label": qsTr("Attention"), "count": page.countForFilter(3) }
                        ]

                        delegate: Button {
                            required property var modelData
                            text: qsTr("%1 · %2").arg(modelData.label).arg(modelData.count)
                            checkable: true
                            checked: page.residencyFilter === modelData.index
                            onClicked: {
                                page.residencyFilter = modelData.index
                                page.rebuildFilteredEntries()
                            }
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                    }

                    Label {
                        text: page.listCountLabel()
                        color: page.textMutedColor
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            visible: page.selectedCount() > 0
            radius: Kirigami.Units.largeSpacing
            color: "#eef5ff"
            border.width: 1
            border.color: "#c8d9ea"

            RowLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.mediumSpacing
                spacing: Kirigami.Units.smallSpacing

                Label {
                    text: qsTr("%1 selected").arg(page.selectedCount())
                    font.bold: true
                    color: "#15314f"
                }

                Button {
                    text: qsTr("Keep on device")
                    icon.name: "emblem-favorite"
                    highlighted: true
                    enabled: page.canManageResidencyActions
                    onClicked: page.runSelectionAction(shellBackend.keepLocalPaths)
                }

                Button {
                    text: qsTr("Free up space")
                    icon.name: "folder-download"
                    enabled: page.canManageResidencyActions
                    onClicked: page.runSelectionAction(shellBackend.makeOnlineOnlyPaths)
                }

                Button {
                    text: qsTr("Retry transfer")
                    icon.name: "view-refresh"
                    enabled: page.canManageResidencyActions
                    onClicked: page.runSelectionAction(shellBackend.retryTransferPaths)
                }

                Item {
                    Layout.fillWidth: true
                }

                Button {
                    text: qsTr("Clear selection")
                    onClicked: page.clearSelection()
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: Kirigami.Units.largeSpacing

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: Kirigami.Units.largeSpacing
                color: page.surfaceColor
                border.width: 1
                border.color: page.lineColor

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 0

                    Rectangle {
                        Layout.fillWidth: true
                        color: page.panelColor
                        border.width: 0
                        implicitHeight: headerRow.implicitHeight + Kirigami.Units.mediumSpacing * 2

                        RowLayout {
                            id: headerRow
                            anchors.fill: parent
                            anchors.margins: Kirigami.Units.mediumSpacing
                            spacing: Kirigami.Units.mediumSpacing

                            Label {
                                Layout.preferredWidth: 320
                                text: qsTr("Name")
                                color: page.textMutedColor
                                font.bold: true
                            }

                            Label {
                                Layout.preferredWidth: 210
                                text: qsTr("State")
                                color: page.textMutedColor
                                font.bold: true
                            }

                            Label {
                                Layout.fillWidth: true
                                text: qsTr("Details")
                                color: page.textMutedColor
                                font.bold: true
                            }

                            Label {
                                Layout.preferredWidth: 140
                                horizontalAlignment: Text.AlignRight
                                text: qsTr("Actions")
                                color: page.textMutedColor
                                font.bold: true
                            }
                        }
                    }

                    Loader {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
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

            Rectangle {
                Layout.preferredWidth: 280
                Layout.fillHeight: true
                radius: Kirigami.Units.largeSpacing
                color: page.surfaceColor
                border.width: 1
                border.color: page.lineColor

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.mediumSpacing

                    Label {
                        text: qsTr("Inspector")
                        color: page.textMutedColor
                        font.bold: true
                    }

                    Kirigami.Heading {
                        Layout.fillWidth: true
                        level: 2
                        wrapMode: Text.WordWrap
                        text: page.selectionSummaryLabel()
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: page.textMutedColor
                        text: page.selectionSummaryBody()
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        radius: Kirigami.Units.largeSpacing
                        color: page.panelColor
                        border.width: 1
                        border.color: page.lineColor

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: Kirigami.Units.mediumSpacing
                            spacing: Kirigami.Units.smallSpacing

                            Label {
                                text: qsTr("Current folder")
                                color: page.textMutedColor
                                font.bold: true
                            }

                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: page.currentPath.length > 0 ? page.currentPath : qsTr("Root")
                            }

                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                color: page.textMutedColor
                                text: qsTr("%1 visible item(s) after filtering.").arg(page.filteredEntries.length)
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        radius: Kirigami.Units.largeSpacing
                        color: page.panelColor
                        border.width: 1
                        border.color: page.lineColor

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: Kirigami.Units.mediumSpacing
                            spacing: Kirigami.Units.smallSpacing

                            Label {
                                text: qsTr("Visibility")
                                color: page.textMutedColor
                                font.bold: true
                            }

                            Repeater {
                                model: [
                                    { "label": qsTr("Online-only"), "value": qsTr("%1").arg(page.countForFilter(1)), "color": "#3d77d9" },
                                    { "label": qsTr("Local"), "value": qsTr("%1").arg(page.countForFilter(2)), "color": "#147a51" },
                                    { "label": qsTr("Attention"), "value": qsTr("%1").arg(page.countForFilter(3)), "color": "#b53b2d" }
                                ]

                                delegate: RowLayout {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    spacing: Kirigami.Units.smallSpacing

                                    Rectangle {
                                        radius: 999
                                        implicitWidth: 10
                                        implicitHeight: 10
                                        color: modelData.color
                                    }

                                    Label {
                                        text: modelData.label
                                        color: page.textMutedColor
                                    }

                                    Item {
                                        Layout.fillWidth: true
                                    }

                                    Label {
                                        text: modelData.value
                                        font.bold: true
                                    }
                                }
                            }

                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                color: page.textMutedColor
                                text: qsTr("Online-only items remain visible and download only when opened or pinned.")
                            }
                        }
                    }

                    Item {
                        Layout.fillHeight: true
                    }

                    Button {
                        Layout.fillWidth: true
                        text: qsTr("Open mounted folder")
                        icon.name: "document-open-folder"
                        enabled: page.canOpenMountedPaths
                        onClicked: shellBackend.openMountLocation()
                    }
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
                text: qsTr("Refreshing files…")
                font.bold: true
            }
        }
    }

    Component {
        id: messageState

        ColumnLayout {
            anchors.centerIn: parent
            width: Math.min(parent.width, 520)
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
                        ? qsTr("Files is waiting for the daemon")
                        : qsTr("The file list could not be loaded")
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
                      ? qsTr("Try a broader term or switch the residency filter back to All.")
                      : qsTr("If you expected online-only files here, run a refresh before assuming the folder is empty.")
            }
        }
    }

    Component {
        id: listState

        ListView {
            clip: true
            model: page.filteredEntries
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { }

            delegate: Rectangle {
                id: rowRoot
                required property var modelData
                readonly property var entry: modelData
                readonly property string entryNameText: page.entryName(entry)

                width: ListView.view.width
                implicitHeight: Math.max(68, rowLayout.implicitHeight + Kirigami.Units.mediumSpacing * 2)
                color: page.isSelected(entry.path) ? page.selectedRowColor : "#ffffff"
                border.width: 0

                Rectangle {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: 4
                    color: page.stateColor(entry)
                }

                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 1
                    color: page.lineColor
                }

                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: function(eventPoint) {
                        page.openContextMenu(entry, rowRoot, eventPoint.position.x, eventPoint.position.y)
                    }
                }

                TapHandler {
                    acceptedButtons: Qt.LeftButton
                    onTapped: function(eventPoint) {
                        if (eventPoint.modifiers & Qt.ControlModifier) {
                            page.toggleSelection(entry.path)
                        } else {
                            page.selectOnly(entry.path)
                        }
                    }
                }

                RowLayout {
                    id: rowLayout
                    anchors.fill: parent
                    anchors.leftMargin: Kirigami.Units.largeSpacing
                    anchors.rightMargin: Kirigami.Units.mediumSpacing
                    anchors.topMargin: Kirigami.Units.smallSpacing
                    anchors.bottomMargin: Kirigami.Units.smallSpacing
                    spacing: Kirigami.Units.mediumSpacing

                    CheckBox {
                        checked: page.isSelected(entry.path)
                        onClicked: page.toggleSelection(entry.path)
                    }

                        RowLayout {
                            Layout.preferredWidth: 320
                            Layout.alignment: Qt.AlignVCenter
                            spacing: Kirigami.Units.smallSpacing

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
                                text: entryNameText
                                font.bold: true
                                enabled: page.canOpenEntry(entry)
                                onClicked: page.showEntry(entry)
                            }

                            Label {
                                Layout.fillWidth: true
                                visible: page.searchActive
                                wrapMode: Text.WordWrap
                                color: page.textMutedColor
                                text: entry.path
                            }
                        }
                    }

                    ColumnLayout {
                        Layout.preferredWidth: 210
                        Layout.alignment: Qt.AlignVCenter
                        spacing: 2

                        Rectangle {
                            radius: 999
                            color: Qt.rgba(page.stateColor(entry).r, page.stateColor(entry).g, page.stateColor(entry).b, 0.14)
                            border.width: 1
                            border.color: Qt.rgba(page.stateColor(entry).r, page.stateColor(entry).g, page.stateColor(entry).b, 0.24)
                            implicitHeight: stateBadgeRow.implicitHeight + Kirigami.Units.smallSpacing * 2
                            implicitWidth: stateBadgeRow.implicitWidth + Kirigami.Units.mediumSpacing * 2

                            RowLayout {
                                id: stateBadgeRow
                                anchors.centerIn: parent
                                spacing: Kirigami.Units.smallSpacing

                                Kirigami.Icon {
                                    source: page.stateIcon(entry)
                                    implicitWidth: Kirigami.Units.iconSizes.small
                                    implicitHeight: Kirigami.Units.iconSizes.small
                                    color: page.stateColor(entry)
                                }

                                Label {
                                    id: stateLabelText
                                    text: page.stateLabel(entry)
                                    color: page.stateColor(entry)
                                    font.bold: true
                                }
                            }
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: page.stateColor(entry)
                            text: page.stateHint(entry)
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignVCenter
                        wrapMode: Text.WordWrap
                        color: page.textMutedColor
                        text: page.detailText(entry)
                    }

                    RowLayout {
                        id: delegateRoot
                        Layout.preferredWidth: 140
                        Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
                        spacing: Kirigami.Units.smallSpacing

                        Button {
                            id: menuButton
                            text: qsTr("...")
                            onClicked: page.openContextMenu(entry, menuButton, width / 2, height)
                        }

                        Button {
                            text: page.quickActionText(entry)
                            icon.name: page.quickActionIcon(entry)
                            enabled: page.canManageResidencyActions
                            onClicked: page.runQuickAction(entry)
                        }
                    }
                }
            }
        }
    }
}
