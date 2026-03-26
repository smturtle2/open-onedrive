import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: page
    title: qsTr("open-onedrive")

    property int currentIndex: 0
    property int lastRecommendedIndex: 0
    readonly property color canvasColor: "#edf3f7"
    readonly property color sidebarColor: "#0f1c28"
    readonly property color sidebarLineColor: Qt.rgba(1, 1, 1, 0.08)
    readonly property color surfaceColor: "#fbfdff"
    readonly property color mutedSurfaceColor: "#f3f7fb"
    readonly property color lineColor: Qt.rgba(11 / 255, 28 / 255, 45 / 255, 0.09)
    readonly property color headingColor: "#102638"
    readonly property color mutedTextColor: "#617283"
    readonly property color stateAccentColor: page.stateAccent()

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
            return qsTr("Service offline")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Connect OneDrive")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Preparing workspace")
        }
        if (shellBackend.appState === "recovery") {
            return qsTr("Attention needed")
        }
        return qsTr("Ready")
    }

    function stateSummary() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Start the background service, then return here to reopen Files.")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Choose the visible folder and complete the browser sign-in.")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Connection, mount, or transfer work is still running.")
        }
        if (shellBackend.appState === "recovery") {
            return qsTr("Use Settings or Logs for repair, then return to Files.")
        }
        if (shellBackend.appState === "running") {
            return qsTr("Online-only and local items are available in the same visible folder.")
        }
        return qsTr("Review the current state or open Files.")
    }

    function pageLabel(index) {
        switch (index) {
        case 0:
            return qsTr("Dashboard")
        case 1:
            return qsTr("Files")
        case 2:
            return qsTr("Settings")
        default:
            return qsTr("Logs")
        }
    }

    function pageDescription(index) {
        switch (index) {
        case 0:
            return qsTr("Queue, sync, and the next action in one place.")
        case 1:
            return qsTr("Browse visible online-only and local items together.")
        case 2:
            return qsTr("Folder path, connection, and recovery controls.")
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
        if (shellBackend.appState === "running") {
            return 1
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
            page.setPage(1)
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
                text: qsTr("Disconnect removes the app-owned sign-in and clears local state stored in %1.").arg(shellBackend.backingDirName)
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.neutralTextColor
                text: qsTr("Online files stay in OneDrive. Use this only when detaching this device or rebuilding local state.")
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
            Layout.preferredWidth: 226
            Layout.maximumWidth: 238
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
                        color: "#d3e0ec"
                        text: page.stateSummary()
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
                            { "index": 0, "label": qsTr("Dashboard"), "icon": "view-dashboard" },
                            { "index": 1, "label": qsTr("Files"), "icon": "folder-open" },
                            { "index": 2, "label": qsTr("Settings"), "icon": "settings-configure" },
                            { "index": 3, "label": qsTr("Logs"), "icon": "view-list-text" }
                        ]

                        delegate: Button {
                            required property var modelData
                            Layout.fillWidth: true
                            flat: true
                            onClicked: page.setPage(modelData.index)

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
                                  : qsTr("Choose a folder in Settings.")
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

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: "#a5bbcf"
                    text: qsTr("Closing the window keeps controls available in the tray.")
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

                            Rectangle {
                                radius: 999
                                color: Qt.rgba(page.stateAccentColor.r, page.stateAccentColor.g, page.stateAccentColor.b, 0.14)
                                border.width: 1
                                border.color: Qt.rgba(page.stateAccentColor.r, page.stateAccentColor.g, page.stateAccentColor.b, 0.28)
                                implicitHeight: stateBadge.implicitHeight + Kirigami.Units.smallSpacing * 2
                                implicitWidth: stateBadge.implicitWidth + Kirigami.Units.largeSpacing

                                Label {
                                    id: stateBadge
                                    anchors.centerIn: parent
                                    text: page.stateLabel()
                                    color: page.stateAccentColor
                                    font.bold: true
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
                                text: qsTr("More")
                                icon.name: "overflow-menu"
                                onClicked: quickMenu.open()
                            }
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: page.mutedTextColor
                            text: shellBackend.statusMessage
                        }

                        Flow {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            Repeater {
                                model: [
                                    { "label": qsTr("Connection"), "value": shellBackend.connectionStateLabel, "accent": "#2d6cdf" },
                                    { "label": qsTr("Filesystem"), "value": shellBackend.mountStateLabel, "accent": page.stateAccent() },
                                    { "label": qsTr("Sync"), "value": shellBackend.syncStateLabel, "accent": shellBackend.syncState === "Error" ? "#bd3f2f" : "#2d6cdf" }
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

                            Button {
                                visible: shellBackend.remoteConfigured
                                text: qsTr("Open Folder")
                                icon.name: "document-open-folder"
                                enabled: shellBackend.mountState === "Running" && shellBackend.effectiveMountPath.length > 0
                                onClicked: shellBackend.openMountLocation()
                            }
                        }
                    }
                }

                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    currentIndex: page.currentIndex

                    DashboardPage {
                        requestDisconnect: page.openDisconnectDialog
                        requestExplorer: function() { page.setPage(1) }
                        requestSetup: function() { page.setPage(2) }
                        requestLogs: function() { page.setPage(3) }
                    }

                    ExplorerPage { }

                    SetupPage {
                        requestDisconnect: page.openDisconnectDialog
                    }

                    LogsPage { }
                }
            }
        }
    }
}
