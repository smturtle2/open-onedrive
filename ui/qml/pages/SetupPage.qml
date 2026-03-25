import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    id: page
    title: shellBackend.remoteConfigured ? qsTr("Recover Mount") : qsTr("Set Up")

    ColumnLayout {
        width: Math.min(parent.width, 760)
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }

        Kirigami.Heading {
            text: shellBackend.remoteConfigured ? qsTr("Reconnect the OneDrive mount") : qsTr("Connect OneDrive with rclone")
            level: 1
        }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: shellBackend.remoteConfigured
                  ? qsTr("The app-owned rclone remote already exists. Choose whether to mount it again, retry a failed mount, or disconnect it completely.")
                  : qsTr("Choose where the OneDrive mount should appear on this machine, then start the browser sign-in flow managed by rclone.")
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: Kirigami.MessageType.Information
            text: qsTr("open-onedrive keeps its own rclone profile under XDG config paths and leaves your default ~/.config/rclone/rclone.conf untouched.")
        }

        MountPathEditor {
            helperText: qsTr("The daemon only writes its own rclone config under XDG config/open-onedrive/rclone/rclone.conf and never touches ~/.config/rclone/rclone.conf.")
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
                          ? qsTr("A custom client ID is already configured in the app config file.")
                          : qsTr("This flow uses rclone's default Microsoft OAuth app. Custom client IDs stay out of the default UI and can be added manually in config.toml.")
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true

            Button {
                text: shellBackend.remoteConfigured ? qsTr("Retry Mount") : qsTr("Connect OneDrive")
                icon.name: shellBackend.remoteConfigured ? "view-refresh" : "network-connect"
                enabled: shellBackend.mountPath.length > 0
                onClicked: shellBackend.remoteConfigured ? shellBackend.retryMount() : shellBackend.beginConnect()
            }

            Button {
                text: qsTr("Mount")
                icon.name: "folder-cloud"
                enabled: shellBackend.canMount
                onClicked: shellBackend.mountRemote()
            }

            Button {
                text: qsTr("Disconnect")
                icon.name: "network-disconnect"
                enabled: shellBackend.remoteConfigured
                onClicked: shellBackend.disconnectRemote()
            }

            Button {
                text: qsTr("Open Mount Folder")
                icon.name: "document-open-folder"
                enabled: shellBackend.effectiveMountPath.length > 0
                onClicked: shellBackend.openMountLocation()
            }
        }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.neutralTextColor
            text: shellBackend.statusMessage
        }
    }
}
