import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: page
    title: qsTr("open-onedrive")

    property int currentIndex: 0
    property bool userSelectedPage: false
    property bool internalPageChange: false

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
            return "#4f7cff"
        }
        return "#3a5a78"
    }

    function stateLabel() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Daemon offline")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Set up OneDrive")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Working")
        }
        if (shellBackend.appState === "recovery") {
            return qsTr("Needs recovery")
        }
        if (shellBackend.appState === "running") {
            return qsTr("Filesystem running")
        }
        return qsTr("Ready to start")
    }

    function stateSummary() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("The background service is not responding. Keep the window open for logs and restart instructions while the tray stays available.")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Choose the visible root folder, start the browser sign-in, and let open-onedrive prepare the local folder on this machine.")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Authentication, startup, or transfer work is in progress. Keep this shell nearby until the visible folder is ready.")
        }
        if (shellBackend.appState === "recovery") {
            return qsTr("One action is needed before normal sync resumes. Repair or retry first, then use Logs if you need more detail.")
        }
        if (shellBackend.appState === "running") {
            return qsTr("The visible root is active. Use Overview for file residency and queue health, or Setup when you need to move the root.")
        }
        return qsTr("OneDrive is connected, but the visible filesystem is not running yet. Start it when you are ready to expose the local folder.")
    }

    function recommendedIndex() {
        if (shellBackend.appState === "welcome" || shellBackend.appState === "recovery") {
            return 1
        }
        if (shellBackend.appState === "daemon-unavailable") {
            return 2
        }
        return 0
    }

    function setPage(index, fromUser) {
        if (page.currentIndex === index) {
            return
        }
        page.internalPageChange = true
        page.currentIndex = index
        page.internalPageChange = false
        if (fromUser) {
            page.userSelectedPage = true
        }
    }

    function syncRecommendedPage(force) {
        if (force || !page.userSelectedPage || page.currentIndex !== 2) {
            page.setPage(page.recommendedIndex(), false)
        }
    }

    function primaryActionText() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("View Logs")
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
        if (shellBackend.remoteConfigured && shellBackend.effectiveMountPath.length > 0) {
            return qsTr("Open Folder")
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
        if (shellBackend.remoteConfigured && shellBackend.effectiveMountPath.length > 0) {
            return "document-open-folder"
        }
        return "view-refresh"
    }

    function primaryActionEnabled() {
        if (shellBackend.appState === "daemon-unavailable") {
            return true
        }
        if (shellBackend.appState === "welcome") {
            return shellBackend.daemonReachable && shellBackend.mountPath.length > 0
        }
        if (shellBackend.needsRemoteRepair) {
            return shellBackend.daemonReachable && shellBackend.mountPath.length > 0
        }
        if (shellBackend.canRetry) {
            return true
        }
        if (shellBackend.canMount) {
            return true
        }
        if (shellBackend.remoteConfigured && shellBackend.effectiveMountPath.length > 0) {
            return true
        }
        return true
    }

    function runPrimaryAction() {
        if (shellBackend.appState === "daemon-unavailable") {
            page.setPage(2, true)
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
        if (shellBackend.remoteConfigured && shellBackend.effectiveMountPath.length > 0) {
            shellBackend.openMountLocation()
            return
        }
        shellBackend.refreshStatus()
    }

    function openDisconnectDialog() {
        disconnectDialog.open()
    }

    Component.onCompleted: syncRecommendedPage(true)

    Connections {
        target: shellBackend

        function onAppStateChanged() {
            page.syncRecommendedPage(false)
        }
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
                text: qsTr("Disconnect removes the app-owned rclone remote, clears the local path-state database, and deletes hydrated bytes stored in %1 under the visible root. Online files in OneDrive stay intact.").arg(shellBackend.backingDirName)
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.neutralTextColor
                text: qsTr("Use this only when you want to rebuild the local state from scratch or fully detach this machine from open-onedrive.")
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        Frame {
            Layout.fillWidth: true
            padding: Kirigami.Units.largeSpacing

            background: Rectangle {
                radius: Kirigami.Units.largeSpacing
                color: Kirigami.Theme.backgroundColor
                border.width: 1
                border.color: page.stateAccent()
                Behavior on color {
                    ColorAnimation { duration: 180 }
                }
            }

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                RowLayout {
                    Layout.fillWidth: true

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Kirigami.Heading {
                            text: page.stateLabel()
                            level: 1
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: page.stateSummary()
                            color: Kirigami.Theme.neutralTextColor
                        }
                    }

                    Rectangle {
                        radius: 999
                        color: page.stateAccent()
                        implicitHeight: stateBadgeLabel.implicitHeight + Kirigami.Units.smallSpacing * 2
                        implicitWidth: stateBadgeLabel.implicitWidth + Kirigami.Units.largeSpacing * 2
                        Behavior on color {
                            ColorAnimation { duration: 180 }
                        }

                        Label {
                            id: stateBadgeLabel
                            anchors.centerIn: parent
                            text: shellBackend.mountStateLabel
                            color: "white"
                            font.bold: true
                        }
                    }
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    type: shellBackend.appState === "recovery" || shellBackend.appState === "daemon-unavailable"
                          ? Kirigami.MessageType.Error
                          : shellBackend.appState === "connecting"
                            ? Kirigami.MessageType.Warning
                            : Kirigami.MessageType.Information
                    text: shellBackend.statusMessage
                }

                RowLayout {
                    Layout.fillWidth: true

                    Item {
                        Layout.fillWidth: true
                    }

                    Button {
                        text: page.primaryActionText()
                        icon.name: page.primaryActionIcon()
                        highlighted: true
                        enabled: page.primaryActionEnabled()
                        onClicked: page.runPrimaryAction()
                    }

                    Button {
                        text: qsTr("Logs")
                        icon.name: "view-list-text"
                        onClicked: page.setPage(2, true)
                    }

                    Button {
                        text: qsTr("Refresh")
                        icon.name: "view-refresh"
                        onClicked: shellBackend.refreshStatus()
                    }
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("Closing the window keeps open-onedrive in the system tray so sync, recovery, and quick controls stay available.")
                }

                RowLayout {
                    Layout.fillWidth: true

                    Button {
                        text: qsTr("Open Folder")
                        icon.name: "document-open-folder"
                        visible: shellBackend.remoteConfigured
                        enabled: shellBackend.effectiveMountPath.length > 0
                        onClicked: shellBackend.openMountLocation()
                    }

                    Button {
                        text: qsTr("Disconnect")
                        icon.name: "network-disconnect"
                        visible: shellBackend.remoteConfigured
                        enabled: shellBackend.daemonReachable
                        onClicked: page.openDisconnectDialog()
                    }
                }
            }
        }

        TabBar {
            Layout.fillWidth: true
            currentIndex: page.currentIndex
            onCurrentIndexChanged: {
                if (!page.internalPageChange) {
                    page.userSelectedPage = true
                    page.currentIndex = currentIndex
                }
            }

            TabButton {
                text: qsTr("Overview")
            }

            TabButton {
                text: qsTr("Setup")
            }

            TabButton {
                text: qsTr("Logs")
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: page.currentIndex

            DashboardPage {
                requestDisconnect: page.openDisconnectDialog
            }
            SetupPage {
                requestDisconnect: page.openDisconnectDialog
            }
            LogsPage { }
        }
    }
}
