import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    id: page
    title: qsTr("Settings")

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
            return qsTr("Manage the visible folder for this device")
        }
        return qsTr("Choose the visible folder and sign in")
    }

    function stageBody() {
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair refreshes only the app-owned OneDrive remote. Existing local backing bytes and path state stay on this device.")
        }
        if (shellBackend.remoteConfigured) {
            return qsTr("This page stays intentionally small: folder path, connection state, and recovery actions only.")
        }
        return qsTr("The first run is short. Pick an empty folder, complete the browser sign-in, then start the visible filesystem.")
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

    ColumnLayout {
        width: Math.min(parent.width, 860)
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
            showCloseButton: false
            type: !shellBackend.daemonReachable
                  ? Kirigami.MessageType.Warning
                  : shellBackend.needsRemoteRepair
                    ? Kirigami.MessageType.Error
                    : Kirigami.MessageType.Information
            text: !shellBackend.daemonReachable
                  ? qsTr("The background service is offline. You can still review the folder path while the daemon comes back.")
                  : shellBackend.needsRemoteRepair
                    ? qsTr("Repair replaces only the app-owned sign-in. It does not delete online files in OneDrive.")
                    : qsTr("open-onedrive keeps its own rclone profile for this app and leaves your regular rclone setup untouched.")
        }

        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing
            color: "#ffffff"
            border.width: 1
            border.color: Qt.rgba(11 / 255, 28 / 255, 45 / 255, 0.09)

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.mediumSpacing

                Label {
                    text: qsTr("Visible folder")
                    color: Kirigami.Theme.neutralTextColor
                    font.bold: true
                }

                MountPathEditor {
                    helperText: qsTr("Choose the OneDrive folder path that should appear in the file manager.")
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing
            color: "#ffffff"
            border.width: 1
            border.color: Qt.rgba(11 / 255, 28 / 255, 45 / 255, 0.09)

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: qsTr("Connection")
                    level: 3
                }

                Repeater {
                    model: [
                        { "label": qsTr("Connection"), "value": shellBackend.connectionStateLabel },
                        { "label": qsTr("Filesystem"), "value": shellBackend.mountStateLabel },
                        { "label": qsTr("Sync"), "value": shellBackend.syncStateLabel }
                    ]

                    delegate: RowLayout {
                        required property var modelData
                        Layout.fillWidth: true

                        Label {
                            text: modelData.label
                            color: Kirigami.Theme.neutralTextColor
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
                        text: qsTr("Start Filesystem")
                        icon.name: "folder-cloud"
                        visible: shellBackend.canMount
                        onClicked: shellBackend.mountRemote()
                    }

                    Button {
                        text: qsTr("Open Folder")
                        icon.name: "document-open-folder"
                        visible: shellBackend.effectiveMountPath.length > 0
                        enabled: shellBackend.effectiveMountPath.length > 0
                        onClicked: shellBackend.openMountLocation()
                    }

                    Button {
                        text: qsTr("Refresh status")
                        icon.name: "view-refresh"
                        onClicked: shellBackend.refreshStatus()
                    }

                    Button {
                        text: qsTr("Disconnect")
                        icon.name: "network-disconnect"
                        visible: shellBackend.remoteConfigured
                        onClicked: requestDisconnect ? requestDisconnect() : shellBackend.disconnectRemote()
                    }
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("Closing the window keeps tray controls alive, so the daemon can continue working independently.")
                }
            }
        }
    }
}
