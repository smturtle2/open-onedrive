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
            return qsTr("Reconnect")
        }
        return qsTr("Connect OneDrive")
    }

    function primaryActionIcon() {
        if (shellBackend.needsRemoteRepair) {
            return "tools-wizard"
        }
        if (shellBackend.remoteConfigured) {
            return "network-connect"
        }
        return "network-connect"
    }

    function stageTitle() {
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair the saved remote profile")
        }
        if (shellBackend.remoteConfigured) {
            return qsTr("Visible folder and connection")
        }
        return qsTr("Choose the visible folder and connect")
    }

    function stageBody() {
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair rebuilds only the app-owned remote profile. Local backing bytes and path state stay on this device.")
        }
        if (shellBackend.remoteConfigured) {
            return qsTr("This page stays intentionally small: folder path plus connection and repair controls.")
        }
        return qsTr("Pick an empty folder, complete the browser sign-in, then return to Dashboard or Files.")
    }

    function runPrimaryAction() {
        if (shellBackend.needsRemoteRepair) {
            shellBackend.repairRemote()
            return
        }
        shellBackend.beginConnect()
    }

    ColumnLayout {
        width: Math.min(parent.width, 760)
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
                  ? qsTr("The background service is offline. Start openonedrived first if actions here do not respond.")
                  : shellBackend.needsRemoteRepair
                    ? qsTr("Repair replaces only the app-owned sign-in. It does not remove files from OneDrive.")
                    : qsTr("open-onedrive keeps its own rclone profile and leaves your regular rclone setup untouched.")
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

                Label {
                    text: qsTr("Connection")
                    color: Kirigami.Theme.neutralTextColor
                    font.bold: true
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
                        text: qsTr("Disconnect")
                        icon.name: "network-disconnect"
                        visible: shellBackend.remoteConfigured
                        onClicked: requestDisconnect ? requestDisconnect() : shellBackend.disconnectRemote()
                    }
                }
            }
        }
    }
}
