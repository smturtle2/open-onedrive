import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: page
    title: qsTr("open-onedrive")

    property int currentIndex: 0
    property int lastRecommendedIndex: 0
    readonly property color canvasColor: "#eef3fa"
    readonly property color surfaceColor: "#ffffff"
    readonly property color mutedSurfaceColor: "#edf2f8"
    readonly property color lineColor: Qt.rgba(10 / 255, 28 / 255, 49 / 255, 0.08)

    function stateAccent() {
        if (shellBackend.appState === "running") {
            return "#1f7a4d"
        }
        if (shellBackend.appState === "connecting") {
            return "#c77700"
        }
        if (shellBackend.appState === "recovery" || shellBackend.appState === "daemon-unavailable") {
            return "#b3261e"
        }
        if (shellBackend.appState === "welcome") {
            return "#3f76ff"
        }
        return "#295c8a"
    }

    function stateLabel() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Background service offline")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Connect OneDrive")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Preparing your folder")
        }
        if (shellBackend.appState === "recovery") {
            return qsTr("Recovery needed")
        }
        if (shellBackend.appState === "running") {
            return qsTr("Visible folder ready")
        }
        return qsTr("Ready to start")
    }

    function stateSummary() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Keep this window open for recovery steps and logs, then start the background service again.")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Choose where the visible OneDrive folder should live, then finish Microsoft sign-in in your browser.")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Sign-in, startup, or transfer work is still in progress. Explorer and logs stay in sync with the same daemon state.")
        }
        if (shellBackend.appState === "recovery") {
            return qsTr("Something needs attention before normal sync resumes. Setup and Logs keep the next recovery step close by.")
        }
        if (shellBackend.appState === "running") {
            return qsTr("Browse files, keep selected items on this device, or return them to online-only mode without leaving the app.")
        }
        return qsTr("The account is connected. Start the filesystem when you are ready to expose the visible OneDrive folder.")
    }

    function recommendedIndex() {
        if (shellBackend.appState === "welcome" || shellBackend.appState === "recovery") {
            return 2
        }
        if (shellBackend.appState === "daemon-unavailable") {
            return 3
        }
        if (shellBackend.remoteConfigured) {
            return 1
        }
        return 0
    }

    function pageLabel(index) {
        switch (index) {
        case 0:
            return qsTr("Overview")
        case 1:
            return qsTr("Explorer")
        case 2:
            return qsTr("Setup")
        default:
            return qsTr("Logs")
        }
    }

    function setPage(index) {
        page.currentIndex = index
    }

    function syncRecommendedPage(force) {
        const nextIndex = page.recommendedIndex()
        const shouldFollow = force || page.currentIndex === 0 || page.currentIndex === page.lastRecommendedIndex
        page.lastRecommendedIndex = nextIndex
        if (shouldFollow && page.currentIndex !== nextIndex) {
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
            return qsTr("Open Explorer")
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
                text: qsTr("Disconnect removes the app-owned sign-in, clears local file-state data, and deletes cached offline bytes stored in %1 under the visible folder. Your online files stay in OneDrive.").arg(shellBackend.backingDirName)
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.neutralTextColor
                text: qsTr("Use this only when you want to fully detach this device or rebuild the local state from scratch.")
            }
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
            Layout.preferredWidth: Math.max(276, Math.min(320, page.width * 0.28))
            radius: Kirigami.Units.largeSpacing * 1.2
            color: page.surfaceColor
            border.width: 1
            border.color: page.lineColor

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.largeSpacing

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Label {
                        text: qsTr("open-onedrive")
                        color: "#335b84"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 1.4
                        font.bold: true
                    }

                    Kirigami.Heading {
                        Layout.fillWidth: true
                        level: 1
                        wrapMode: Text.WordWrap
                        color: "#11283f"
                        text: page.stateLabel()
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: "#5e6f82"
                        text: page.stateSummary()
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    radius: Kirigami.Units.largeSpacing
                    color: page.mutedSurfaceColor
                    border.width: 1
                    border.color: page.lineColor

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.mediumSpacing
                        spacing: Kirigami.Units.smallSpacing

                        RowLayout {
                            Layout.fillWidth: true

                            Rectangle {
                                radius: 999
                                color: page.stateAccent()
                                implicitWidth: stateBadge.implicitWidth + Kirigami.Units.largeSpacing
                                implicitHeight: stateBadge.implicitHeight + Kirigami.Units.smallSpacing * 2

                                Label {
                                    id: stateBadge
                                    anchors.centerIn: parent
                                    text: shellBackend.mountStateLabel
                                    color: "white"
                                    font.bold: true
                                }
                            }

                            Item {
                                Layout.fillWidth: true
                            }

                            Label {
                                text: qsTr("Recommended: %1").arg(page.pageLabel(page.recommendedIndex()))
                                color: "#527295"
                            }
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: "#3d5671"
                            text: shellBackend.statusMessage
                        }
                    }
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
                            text: qsTr("Logs")
                            icon.name: "view-list-text"
                            onClicked: page.setPage(3)
                        }

                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Refresh")
                            icon.name: "view-refresh"
                            onClicked: shellBackend.refreshStatus()
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    radius: Kirigami.Units.largeSpacing
                    color: page.mutedSurfaceColor
                    border.width: 1
                    border.color: page.lineColor

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.mediumSpacing
                        spacing: Kirigami.Units.mediumSpacing

                        Label {
                            text: qsTr("Workspace")
                            color: "#62778f"
                            font.bold: true
                        }

                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Overview")
                            icon.name: "view-dashboard"
                            highlighted: page.currentIndex === 0
                            onClicked: page.setPage(0)
                        }

                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Explorer")
                            icon.name: "folder-open"
                            highlighted: page.currentIndex === 1
                            onClicked: page.setPage(1)
                        }

                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Setup")
                            icon.name: "settings-configure"
                            highlighted: page.currentIndex === 2
                            onClicked: page.setPage(2)
                        }

                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Logs")
                            icon.name: "view-list-text"
                            highlighted: page.currentIndex === 3
                            onClicked: page.setPage(3)
                        }

                        Item {
                            Layout.fillHeight: true
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            radius: Kirigami.Units.mediumSpacing
                            color: "#ffffff"
                            border.width: 1
                            border.color: page.lineColor

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: Kirigami.Units.mediumSpacing
                                spacing: Kirigami.Units.smallSpacing

                                Label {
                                    text: qsTr("Visible folder")
                                    color: "#62778f"
                                }

                                Label {
                                    Layout.fillWidth: true
                                    wrapMode: Text.WordWrap
                                    color: "#15314f"
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
                    }
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: "#5e6f82"
                    text: qsTr("Closing the window keeps open-onedrive in the system tray so recovery, sync, and file actions stay available.")
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

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Kirigami.Units.largeSpacing * 1.2
            color: page.surfaceColor
            border.width: 1
            border.color: page.lineColor

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.mediumSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.mediumSpacing

                    Kirigami.Heading {
                        text: page.pageLabel(page.currentIndex)
                        level: 2
                    }

                    Item {
                        Layout.fillWidth: true
                    }

                    Button {
                        text: qsTr("Open Folder")
                        icon.name: "document-open-folder"
                        visible: shellBackend.remoteConfigured
                        enabled: shellBackend.mountState === "Running" && shellBackend.effectiveMountPath.length > 0
                        onClicked: shellBackend.openMountLocation()
                    }

                    Button {
                        text: qsTr("Explorer")
                        icon.name: "folder-open"
                        visible: shellBackend.remoteConfigured
                        enabled: shellBackend.daemonReachable
                        onClicked: page.setPage(1)
                    }
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    showCloseButton: false
                    type: shellBackend.appState === "recovery" || shellBackend.appState === "daemon-unavailable"
                          ? Kirigami.MessageType.Error
                          : shellBackend.appState === "connecting"
                            ? Kirigami.MessageType.Warning
                            : Kirigami.MessageType.Information
                    text: shellBackend.statusMessage
                }

                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    currentIndex: page.currentIndex

                    DashboardPage {
                        requestDisconnect: page.openDisconnectDialog
                        requestExplorer: function() {
                            page.setPage(1)
                        }
                        requestSetup: function() {
                            page.setPage(2)
                        }
                        requestLogs: function() {
                            page.setPage(3)
                        }
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
