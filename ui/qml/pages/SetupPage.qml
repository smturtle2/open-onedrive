import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    id: page
    title: shellBackend.remoteConfigured ? "Recover Mount" : "Set Up"

    ColumnLayout {
        width: Math.min(parent.width, 760)
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }

        Kirigami.Heading {
            text: shellBackend.remoteConfigured ? "Reconnect the OneDrive mount" : "Connect OneDrive with rclone"
            level: 1
        }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: shellBackend.remoteConfigured
                  ? "The app-owned rclone remote already exists. Choose whether to mount it again, retry a failed mount, or disconnect it completely."
                  : "Choose where the OneDrive mount should appear on this machine, then start the browser sign-in flow managed by rclone."
        }

        MountPathEditor {
            helperText: "The daemon only writes its own rclone config under XDG config/open-onedrive/rclone/rclone.conf and never touches ~/.config/rclone/rclone.conf."
        }

        Frame {
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: "Connection"
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: shellBackend.customClientIdConfigured
                          ? "A custom client ID is already configured in the app config file."
                          : "This flow uses rclone's default Microsoft OAuth app. Custom client IDs stay out of the default UI and can be added manually in config.toml."
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true

            Button {
                text: shellBackend.remoteConfigured ? "Retry Mount" : "Connect OneDrive"
                icon.name: shellBackend.remoteConfigured ? "view-refresh" : "network-connect"
                enabled: shellBackend.mountPath.length > 0
                onClicked: shellBackend.remoteConfigured ? shellBackend.retryMount() : shellBackend.beginConnect()
            }

            Button {
                text: "Mount"
                icon.name: "folder-cloud"
                enabled: shellBackend.remoteConfigured
                onClicked: shellBackend.mountRemote()
            }

            Button {
                text: "Disconnect"
                icon.name: "network-disconnect"
                enabled: shellBackend.remoteConfigured
                onClicked: shellBackend.disconnectRemote()
            }

            Button {
                text: "Open Mount Folder"
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
