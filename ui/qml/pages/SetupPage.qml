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
             ? qsTr("Recover Filesystem")
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
        width: Math.min(parent.width, 760)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }

        Kirigami.Heading {
            text: shellBackend.needsRemoteRepair
                  ? qsTr("Repair the app-owned OneDrive profile")
                  : shellBackend.remoteConfigured
                    ? qsTr("Reconnect the OneDrive root")
                    : qsTr("Connect OneDrive with rclone")
            level: 1
        }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: shellBackend.needsRemoteRepair
                  ? qsTr("This machine still has an older saved sign-in. Repair Remote rebuilds that sign-in, keeps your hydrated files on this device, and restarts the browser flow.")
                  : shellBackend.remoteConfigured
                    ? qsTr("The account is already connected. Use this page to move the visible root, retry the filesystem, or disconnect this device cleanly.")
                    : qsTr("Choose where the visible OneDrive root folder should appear on this machine, then start sign-in in your browser.")
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: !shellBackend.daemonReachable
                  ? Kirigami.MessageType.Warning
                  : shellBackend.needsRemoteRepair
                    ? Kirigami.MessageType.Error
                    : Kirigami.MessageType.Information
            text: !shellBackend.daemonReachable
                  ? qsTr("The daemon is not reachable yet. You can still review the root path and switch to Logs while the service comes back.")
                  : shellBackend.needsRemoteRepair
                    ? qsTr("Repair replaces only the saved sign-in for this app. It does not wipe the hydrated local cache or your saved file state.")
                    : qsTr("open-onedrive keeps its own private sign-in for this app and leaves your normal rclone setup untouched.")
        }

        MountPathEditor {
            helperText: qsTr("open-onedrive stores its own sign-in details for this app only and never takes over your normal rclone configuration.")
        }

        Frame {
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: qsTr("Connection")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: shellBackend.customClientIdConfigured
                          ? qsTr("A custom client ID is already configured for this machine.")
                          : qsTr("The default setup uses rclone's Microsoft app. Advanced client ID changes stay in config.toml instead of the first-run UI.")
                }
            }
        }

        Frame {
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: qsTr("Actions")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: shellBackend.remoteConfigured
                          ? qsTr("Keep recovery actions compact here. Use Overview for runtime health and Explorer for per-file residency control.")
                          : qsTr("Set the visible root first, then start the browser sign-in. Additional recovery actions stay under the overflow menu.")
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
                        text: qsTr("Open Root Folder")
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

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.neutralTextColor
            text: shellBackend.statusMessage
        }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.neutralTextColor
            text: qsTr("Closing the window keeps open-onedrive in the system tray. You can return later without stopping the daemon.")
        }
    }
}
