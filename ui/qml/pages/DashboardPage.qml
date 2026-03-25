import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    title: "Dashboard"

    ColumnLayout {
        width: Math.min(parent.width, 960)
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }

        RowLayout {
            Layout.fillWidth: true

            Kirigami.Heading {
                text: "open-onedrive"
                level: 1
            }

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: "Refresh"
                onClicked: shellBackend.refreshStatus()
            }

            Button {
                text: "Unmount"
                onClicked: shellBackend.unmountRemote()
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width > 720 ? 4 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            StatusCard {
                Layout.fillWidth: true
                title: "Backend"
                value: "rclone"
                description: shellBackend.rcloneVersion.length > 0 ? shellBackend.rcloneVersion : "Version pending"
            }

            StatusCard {
                Layout.fillWidth: true
                title: "Mount State"
                value: shellBackend.mountState
                description: shellBackend.statusMessage
            }

            StatusCard {
                Layout.fillWidth: true
                title: "Mount Path"
                value: shellBackend.mountPath
                description: "Host filesystem path exposed by rclone mount"
            }

            StatusCard {
                Layout.fillWidth: true
                title: "Cache"
                value: shellBackend.cacheUsageLabel
                description: "App-owned rclone VFS cache usage"
            }
        }

        Frame {
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: "Quick Actions"
                    level: 3
                }

                RowLayout {
                    Layout.fillWidth: true

                    Button {
                        text: "Open Mount Location"
                        onClicked: shellBackend.openMountLocation()
                    }

                    Button {
                        text: "Retry Mount"
                        onClicked: shellBackend.retryMount()
                    }

                    Button {
                        text: "Disconnect"
                        onClicked: shellBackend.disconnectRemote()
                    }
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: shellBackend.lastLogLine.length > 0
                          ? shellBackend.lastLogLine
                          : "Recent rclone output will appear here."
                }
            }
        }
    }
}
