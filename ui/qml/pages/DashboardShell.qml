import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: page
    title: qsTr("open-onedrive")

    property int currentIndex: 0
    property int lastRecommendedIndex: 0
    readonly property color canvasColor: "#edf2f7"
    readonly property color sidebarColor: "#12202d"
    readonly property color sidebarLineColor: Qt.rgba(1, 1, 1, 0.08)
    readonly property color surfaceColor: "#fbfdff"
    readonly property color mutedSurfaceColor: "#f2f6fa"
    readonly property color lineColor: Qt.rgba(11 / 255, 28 / 255, 45 / 255, 0.09)
    readonly property color headingColor: "#102538"
    readonly property color mutedTextColor: "#607082"

    function stateAccent() {
        if (shellBackend.appState === "running") {
            return "#1f7a4d"
        }
        if (shellBackend.appState === "connecting") {
            return "#cf8a1c"
        }
        if (shellBackend.appState === "recovery" || shellBackend.appState === "daemon-unavailable") {
            return "#bd3f2f"
        }
        if (shellBackend.appState === "welcome") {
            return "#2d6cdf"
        }
        return "#275b8f"
    }

    function stateLabel() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Background service offline")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Connect OneDrive")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Preparing workspace")
        }
        if (shellBackend.appState === "recovery") {
            return qsTr("Recovery needed")
        }
        if (shellBackend.appState === "running") {
            return qsTr("Files ready")
        }
        return qsTr("Ready")
    }

    function stateSummary() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Use Logs to inspect the daemon, then bring the background service back before browsing files.")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Choose the visible folder and finish Microsoft sign-in. Files becomes the main workspace after setup.")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Startup or transfer work is still running. The file list and tray reflect the same daemon state.")
        }
        if (shellBackend.appState === "recovery") {
            return qsTr("Recovery actions stay in Setup and Logs while Files keeps the current residency view visible.")
        }
        if (shellBackend.appState === "running") {
            return qsTr("Browse online-only and local items side by side, then keep or release them from the same list.")
        }
        return qsTr("Refresh status or open the visible folder.")
    }

    function pageLabel(index) {
        switch (index) {
        case 0:
            return qsTr("Files")
        case 1:
            return qsTr("Activity")
        case 2:
            return qsTr("Setup")
        default:
            return qsTr("Logs")
        }
    }

    function pageDescription(index) {
        switch (index) {
        case 0:
            return qsTr("Browse, filter, and change residency without leaving the app.")
        case 1:
            return qsTr("Compact status, queue, and sync health.")
        case 2:
            return qsTr("Folder path, recovery steps, and connection controls.")
        default:
            return qsTr("Recent daemon and rclone output.")
        }
    }

    function recommendedIndex() {
        if (shellBackend.appState === "welcome" || shellBackend.appState === "recovery") {
            return 2
        }
        if (shellBackend.appState === "daemon-unavailable") {
            return 3
        }
        return 0
    }

    function setPage(index) {
        page.currentIndex = index
    }

    function syncRecommendedPage(force) {
        const nextIndex = page.recommendedIndex()
        const shouldFollow = force || page.currentIndex === page.lastRecommendedIndex
        page.lastRecommendedIndex = nextIndex
        if (shouldFollow) {
            page.currentIndex = nextIndex
        }
    }

    function primaryActionText() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Open Logs")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Connect OneDrive")
        }
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair Remote")
        }
        if (shellBackend.canRetry) {
            return qsTr("Retry Filesystem")
        }
        if (shellBackend.canMount) {
            return qsTr("Start Filesystem")
        }
        if (shellBackend.remoteConfigured) {
            return qsTr("Open Files")
        }
        return qsTr("Refresh")
    }

    function primaryActionIcon() {
        if (shellBackend.appState === "daemon-unavailable") {
            return "view-list-text"
        }
        if (shellBackend.appState === "welcome") {
            return "network-connect"
        }
        if (shellBackend.needsRemoteRepair) {
            return "tools-wizard"
        }
        if (shellBackend.canRetry) {
            return "view-refresh"
        }
        if (shellBackend.canMount) {
            return "folder-cloud"
        }
        if (shellBackend.remoteConfigured) {
            return "folder-open"
        }
        return "view-refresh"
    }

    function primaryActionEnabled() {
        if (shellBackend.appState === "welcome" || shellBackend.needsRemoteRepair) {
            return shellBackend.daemonReachable && shellBackend.mountPath.length > 0
        }
        if (shellBackend.appState === "daemon-unavailable") {
            return true
        }
        return shellBackend.daemonReachable
    }

    function runPrimaryAction() {
        if (shellBackend.appState === "daemon-unavailable") {
            page.setPage(3)
            return
        }
        if (shellBackend.appState === "welcome") {
            shellBackend.beginConnect()
            return
        }
        if (shellBackend.needsRemoteRepair) {
            shellBackend.repairRemote()
            return
        }
        if (shellBackend.canRetry) {
            shellBackend.retryMount()
            return
        }
        if (shellBackend.canMount) {
            shellBackend.mountRemote()
            return
        }
        if (shellBackend.remoteConfigured) {
            page.setPage(0)
            return
        }
        shellBackend.refreshStatus()
    }

    function openDisconnectDialog() {
        disconnectDialog.open()
    }

    Dialog {
        id: disconnectDialog
        title: qsTr("Disconnect OneDrive")
        modal: true
        standardButtons: Dialog.Cancel | Dialog.Ok

        onAccepted: shellBackend.disconnectRemote()

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Disconnect removes the app-owned sign-in, clears local file-state data, and deletes cached offline bytes stored in %1 under the visible folder.").arg(shellBackend.backingDirName)
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.neutralTextColor
                text: qsTr("Online files stay in OneDrive. Use this only when you want to fully detach this device or rebuild local state from scratch.")
            }
        }
    }

    Menu {
        id: quickMenu

        MenuItem {
            text: qsTr("Refresh status")
            icon.name: "view-refresh"
            onTriggered: shellBackend.refreshStatus()
        }

        MenuItem {
            text: qsTr("Start filesystem")
            icon.name: "folder-cloud"
            visible: shellBackend.canMount
            onTriggered: shellBackend.mountRemote()
        }

        MenuItem {
            text: qsTr("Stop filesystem")
            icon.name: "media-eject"
            visible: shellBackend.canUnmount
            onTriggered: shellBackend.unmountRemote()
        }

        MenuItem {
            text: qsTr("Retry filesystem")
            icon.name: "view-refresh"
            visible: shellBackend.canRetry
            onTriggered: shellBackend.retryMount()
        }

        MenuItem {
            text: qsTr("Pause sync")
            icon.name: "media-playback-pause"
            visible: shellBackend.canPauseSync
            onTriggered: shellBackend.pauseSync()
        }

        MenuItem {
            text: qsTr("Resume sync")
            icon.name: "media-playback-start"
            visible: shellBackend.canResumeSync
            onTriggered: shellBackend.resumeSync()
        }

        MenuSeparator { }

        MenuItem {
            text: qsTr("Disconnect")
            icon.name: "network-disconnect"
            visible: shellBackend.remoteConfigured
            onTriggered: page.openDisconnectDialog()
        }
    }

    Component.onCompleted: page.syncRecommendedPage(true)

    Connections {
        target: shellBackend

        function onAppStateChanged() {
            page.syncRecommendedPage(false)
        }
    }

    Rectangle {
        anchors.fill: parent
        z: -1
        color: page.canvasColor
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 250
            Layout.maximumWidth: 278
            radius: Kirigami.Units.largeSpacing * 1.1
            color: page.sidebarColor
            border.width: 1
            border.color: page.sidebarLineColor

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.largeSpacing

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Label {
                        text: qsTr("open-onedrive")
                        color: "#b8d4ee"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 1.4
                        font.bold: true
                    }

                    Kirigami.Heading {
                        Layout.fillWidth: true
                        level: 2
                        wrapMode: Text.WordWrap
                        color: "white"
                        text: page.stateLabel()
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: "#d0dfed"
                        text: page.stateSummary()
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    radius: Kirigami.Units.largeSpacing
                    color: Qt.rgba(1, 1, 1, 0.06)
                    border.width: 1
                    border.color: page.sidebarLineColor

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.mediumSpacing
                        spacing: Kirigami.Units.smallSpacing

                        Rectangle {
                            radius: 999
                            color: page.stateAccent()
                            implicitHeight: stateBadge.implicitHeight + Kirigami.Units.smallSpacing * 2
                            implicitWidth: stateBadge.implicitWidth + Kirigami.Units.largeSpacing

                            Label {
                                id: stateBadge
                                anchors.centerIn: parent
                                color: "white"
                                font.bold: true
                                text: shellBackend.mountStateLabel
                            }
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: "#d9e6f2"
                            text: shellBackend.statusMessage
                        }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Label {
                        text: qsTr("Workspace")
                        color: "#a5bbcf"
                        font.bold: true
                    }

                    Repeater {
                        model: [
                            { "index": 0, "label": qsTr("Files"), "icon": "folder-open" },
                            { "index": 1, "label": qsTr("Activity"), "icon": "view-dashboard" },
                            { "index": 2, "label": qsTr("Setup"), "icon": "settings-configure" },
                            { "index": 3, "label": qsTr("Logs"), "icon": "view-list-text" }
                        ]

                        delegate: Button {
                            required property var modelData
                            Layout.fillWidth: true
                            text: modelData.label
                            icon.name: modelData.icon
                            flat: page.currentIndex !== modelData.index
                            highlighted: page.currentIndex === modelData.index

                            background: Rectangle {
                                radius: Kirigami.Units.mediumSpacing
                                color: page.currentIndex === modelData.index ? "#f5f9ff" : "transparent"
                                border.width: page.currentIndex === modelData.index ? 1 : 0
                                border.color: page.currentIndex === modelData.index ? "#bed0e1" : "transparent"
                            }

                            contentItem: RowLayout {
                                spacing: Kirigami.Units.smallSpacing

                                Kirigami.Icon {
                                    source: modelData.icon
                                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                                    implicitHeight: Kirigami.Units.iconSizes.smallMedium
                                    color: page.currentIndex === modelData.index ? "#0f2c44" : "#d2dfeb"
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.label
                                    color: page.currentIndex === modelData.index ? "#0f2c44" : "#ecf3fb"
                                    font.bold: page.currentIndex === modelData.index
                                }
                            }

                            onClicked: page.setPage(modelData.index)
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    radius: Kirigami.Units.largeSpacing
                    color: Qt.rgba(1, 1, 1, 0.05)
                    border.width: 1
                    border.color: page.sidebarLineColor

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.mediumSpacing
                        spacing: Kirigami.Units.smallSpacing

                        Label {
                            text: qsTr("Visible folder")
                            color: "#a5bbcf"
                            font.bold: true
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: "#edf4fb"
                            text: shellBackend.effectiveMountPath.length > 0
                                  ? shellBackend.effectiveMountPath
                                  : qsTr("Choose a root folder in Setup.")
                        }

                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Open Folder")
                            icon.name: "document-open-folder"
                            enabled: shellBackend.mountState === "Running" && shellBackend.effectiveMountPath.length > 0
                            onClicked: shellBackend.openMountLocation()
                        }
                    }
                }

                Item {
                    Layout.fillHeight: true
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Button {
                        Layout.fillWidth: true
                        text: page.primaryActionText()
                        icon.name: page.primaryActionIcon()
                        highlighted: true
                        enabled: page.primaryActionEnabled()
                        onClicked: page.runPrimaryAction()
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Refresh")
                            icon.name: "view-refresh"
                            onClicked: shellBackend.refreshStatus()
                        }

                        Button {
                            Layout.fillWidth: true
                            text: qsTr("More")
                            icon.name: "overflow-menu"
                            onClicked: quickMenu.open()
                        }
                    }

                    Button {
                        Layout.fillWidth: true
                        visible: shellBackend.remoteConfigured
                        text: qsTr("Disconnect")
                        icon.name: "network-disconnect"
                        onClicked: page.openDisconnectDialog()
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Kirigami.Units.largeSpacing * 1.1
            color: page.surfaceColor
            border.width: 1
            border.color: page.lineColor

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.largeSpacing

                Rectangle {
                    Layout.fillWidth: true
                    radius: Kirigami.Units.largeSpacing
                    color: page.mutedSurfaceColor
                    border.width: 1
                    border.color: page.lineColor

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.mediumSpacing
                        spacing: Kirigami.Units.mediumSpacing

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.largeSpacing

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                Label {
                                    text: page.pageLabel(page.currentIndex)
                                    color: page.mutedTextColor
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 1.1
                                    font.bold: true
                                }

                                Kirigami.Heading {
                                    Layout.fillWidth: true
                                    level: 2
                                    wrapMode: Text.WordWrap
                                    color: page.headingColor
                                    text: page.pageDescription(page.currentIndex)
                                }
                            }

                            Button {
                                text: page.primaryActionText()
                                icon.name: page.primaryActionIcon()
                                highlighted: true
                                enabled: page.primaryActionEnabled()
                                onClicked: page.runPrimaryAction()
                            }

                            Button {
                                text: qsTr("Open Folder")
                                icon.name: "document-open-folder"
                                visible: shellBackend.remoteConfigured
                                enabled: shellBackend.mountState === "Running" && shellBackend.effectiveMountPath.length > 0
                                onClicked: shellBackend.openMountLocation()
                            }
                        }

                        Flow {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            Repeater {
                                model: [
                                    { "label": qsTr("Connection"), "value": shellBackend.connectionStateLabel, "accent": "#2d6cdf" },
                                    { "label": qsTr("Filesystem"), "value": shellBackend.mountStateLabel, "accent": page.stateAccent() },
                                    { "label": qsTr("Sync"), "value": shellBackend.syncStateLabel, "accent": shellBackend.syncState === "Error" ? "#bd3f2f" : "#2d6cdf" },
                                    { "label": qsTr("Queue"), "value": qsTr("%1 pending").arg(shellBackend.queueDepth), "accent": "#7b5e15" }
                                ]

                                delegate: Rectangle {
                                    required property var modelData
                                    radius: 999
                                    color: "#ffffff"
                                    border.width: 1
                                    border.color: page.lineColor
                                    implicitHeight: chipRow.implicitHeight + Kirigami.Units.smallSpacing * 2
                                    implicitWidth: chipRow.implicitWidth + Kirigami.Units.mediumSpacing * 2

                                    RowLayout {
                                        id: chipRow
                                        anchors.centerIn: parent
                                        spacing: Kirigami.Units.smallSpacing

                                        Rectangle {
                                            radius: 999
                                            implicitWidth: 8
                                            implicitHeight: 8
                                            color: modelData.accent
                                        }

                                        Label {
                                            text: modelData.label
                                            color: page.mutedTextColor
                                        }

                                        Label {
                                            text: modelData.value
                                            color: page.headingColor
                                            font.bold: true
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    currentIndex: page.currentIndex

                    ExplorerPage { }

                    DashboardPage {
                        requestDisconnect: page.openDisconnectDialog
                        requestExplorer: function() { page.setPage(0) }
                        requestSetup: function() { page.setPage(2) }
                        requestLogs: function() { page.setPage(3) }
                    }

                    SetupPage {
                        requestDisconnect: page.openDisconnectDialog
                    }

                    LogsPage { }
                }
            }
        }
    }
}
