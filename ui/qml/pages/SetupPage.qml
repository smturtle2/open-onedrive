import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    id: page
    title: shellBackend.needsRemoteRepair
           ? qsTr("Repair Remote")
           : shellBackend.remoteConfigured
             ? qsTr("Setup")
             : qsTr("Set Up")

    property var requestDisconnect: null

    function primaryActionText() {
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair Remote")
        }
        if (shellBackend.remoteConfigured) {
            return qsTr("Retry Filesystem")
        }
        return qsTr("Connect OneDrive")
    }

    function primaryActionIcon() {
        if (shellBackend.needsRemoteRepair) {
            return "tools-wizard"
        }
        if (shellBackend.remoteConfigured) {
            return "view-refresh"
        }
        return "network-connect"
    }

    function stageTitle() {
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair the saved sign-in")
        }
        if (shellBackend.remoteConfigured) {
            return qsTr("Manage the visible folder and recovery actions")
        }
        return qsTr("Choose the visible folder and sign in")
    }

    function stageBody() {
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair replaces only the app-owned OneDrive sign-in. Offline bytes and local file state remain on this device.")
        }
        if (shellBackend.remoteConfigured) {
            return qsTr("Use this page when you need to move the visible folder, restart the filesystem, or disconnect the device cleanly.")
        }
        return qsTr("The setup flow is short: choose an empty folder, finish the browser sign-in, then let the filesystem expose OneDrive as a normal folder.")
    }

    function runPrimaryAction() {
        if (shellBackend.needsRemoteRepair) {
            shellBackend.repairRemote()
            return
        }
        if (shellBackend.remoteConfigured) {
            shellBackend.retryMount()
            return
        }
        shellBackend.beginConnect()
    }

    Menu {
        id: actionMenu

        MenuItem {
            text: qsTr("Start filesystem")
            icon.name: "folder-cloud"
            visible: shellBackend.canMount
            onTriggered: shellBackend.mountRemote()
        }

        MenuItem {
            text: qsTr("Refresh status")
            icon.name: "view-refresh"
            onTriggered: shellBackend.refreshStatus()
        }

        MenuSeparator { }

        MenuItem {
            text: qsTr("Disconnect")
            icon.name: "network-disconnect"
            visible: shellBackend.remoteConfigured
            onTriggered: requestDisconnect ? requestDisconnect() : shellBackend.disconnectRemote()
        }
    }

    ColumnLayout {
        width: Math.min(parent.width, 920)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.smallSpacing
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Heading {
                Layout.fillWidth: true
                level: 1
                wrapMode: Text.WordWrap
                text: page.stageTitle()
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.neutralTextColor
                text: page.stageBody()
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: !shellBackend.daemonReachable
                  ? Kirigami.MessageType.Warning
                  : shellBackend.needsRemoteRepair
                    ? Kirigami.MessageType.Error
                    : Kirigami.MessageType.Information
            showCloseButton: false
            text: !shellBackend.daemonReachable
                  ? qsTr("The background service is offline. You can still review the folder path here and return to Logs while it comes back.")
                  : shellBackend.needsRemoteRepair
                    ? qsTr("Repair refreshes only the app-owned sign-in. It does not delete online files in OneDrive.")
                    : qsTr("open-onedrive keeps its own rclone profile for this app and leaves your regular rclone setup untouched.")
        }

        Frame {
            Layout.fillWidth: true
            padding: Kirigami.Units.largeSpacing

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Label {
                    text: qsTr("Folder path")
                    color: Kirigami.Theme.neutralTextColor
                    font.bold: true
                }

                MountPathEditor {
                    helperText: qsTr("Pick an empty folder that should appear as your normal OneDrive path on this device.")
                }
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width > 860 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            Frame {
                Layout.fillWidth: true
                padding: Kirigami.Units.largeSpacing

                ColumnLayout {
                    anchors.fill: parent
                    spacing: Kirigami.Units.mediumSpacing

                    Kirigami.Heading {
                        text: qsTr("Next step")
                        level: 3
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: Kirigami.Theme.neutralTextColor
                        text: shellBackend.remoteConfigured
                              ? qsTr("Retry or restart the filesystem here. Use Files for residency changes once the folder is ready.")
                              : qsTr("Set the visible folder first, then begin the browser sign-in. Files becomes the main workspace after setup.")
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Button {
                            text: page.primaryActionText()
                            icon.name: page.primaryActionIcon()
                            highlighted: true
                            enabled: shellBackend.daemonReachable && shellBackend.mountPath.length > 0
                            onClicked: page.runPrimaryAction()
                        }

                        Button {
                            text: qsTr("Open Folder")
                            icon.name: "document-open-folder"
                            visible: shellBackend.effectiveMountPath.length > 0
                            enabled: shellBackend.effectiveMountPath.length > 0
                            onClicked: shellBackend.openMountLocation()
                        }

                        Button {
                            text: qsTr("More actions")
                            icon.name: "overflow-menu"
                            onClicked: actionMenu.open()
                        }
                    }
                }
            }

            Frame {
                Layout.fillWidth: true
                padding: Kirigami.Units.largeSpacing

                ColumnLayout {
                    anchors.fill: parent
                    spacing: Kirigami.Units.mediumSpacing

                    Kirigami.Heading {
                        text: qsTr("Notes")
                        level: 3
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: Kirigami.Theme.neutralTextColor
                        text: shellBackend.customClientIdConfigured
                              ? qsTr("A custom Microsoft client ID is already configured for this device.")
                              : qsTr("The default setup uses rclone's Microsoft app. Advanced client changes stay in config.toml rather than the first-run UI.")
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: Kirigami.Theme.neutralTextColor
                        text: qsTr("Closing the window keeps tray controls alive, so the daemon can continue running independently.")
                    }
                }
            }
        }
    }
}
