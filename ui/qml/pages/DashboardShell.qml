import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: page
    title: qsTr("open-onedrive")

    property int currentIndex: 0

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
            return qsTr("The background service is not responding yet. Logs stay available, and you can retry once the daemon is back.")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Choose a visible root folder, start the browser sign-in, and keep the shell open while the daemon writes its app-owned rclone profile.")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Authentication, startup, or transfer work is in progress. Let the daemon finish before switching residency on the same files.")
        }
        if (shellBackend.appState === "recovery") {
            return qsTr("Use the overview controls to retry the filesystem, inspect recent logs, or disconnect and rebuild the local state.")
        }
        if (shellBackend.appState === "running") {
            return qsTr("The visible root is live. Use Overview for residency and sync controls, or Setup to change the root path safely.")
        }
        return qsTr("OneDrive is connected, but the visible filesystem is not running yet. Start it from Overview when you are ready.")
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

                    Button {
                        text: qsTr("Overview")
                        icon.name: "view-dashboard"
                        onClicked: page.currentIndex = 0
                    }

                    Button {
                        text: qsTr("Setup")
                        icon.name: "settings-configure"
                        onClicked: page.currentIndex = 1
                    }

                    Button {
                        text: qsTr("Logs")
                        icon.name: "view-list-text"
                        onClicked: page.currentIndex = 2
                    }

                    Item {
                        Layout.fillWidth: true
                    }

                    Button {
                        text: qsTr("Refresh")
                        icon.name: "view-refresh"
                        onClicked: shellBackend.refreshStatus()
                    }

                    Button {
                        text: shellBackend.remoteConfigured ? qsTr("Open Folder") : qsTr("Connect")
                        icon.name: shellBackend.remoteConfigured ? "document-open-folder" : "network-connect"
                        enabled: shellBackend.remoteConfigured
                                 ? shellBackend.effectiveMountPath.length > 0
                                 : shellBackend.daemonReachable && shellBackend.mountPath.length > 0
                        onClicked: shellBackend.remoteConfigured ? shellBackend.openMountLocation() : shellBackend.beginConnect()
                    }
                }
            }
        }

        TabBar {
            Layout.fillWidth: true
            currentIndex: page.currentIndex
            onCurrentIndexChanged: page.currentIndex = currentIndex

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
