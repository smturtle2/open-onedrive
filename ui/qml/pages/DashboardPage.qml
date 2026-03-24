import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

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
                text: "Pause"
                onClicked: shellBackend.pauseSync()
            }

            Button {
                text: "Refresh"
                onClicked: shellBackend.refreshStatus()
            }

            Button {
                text: "Resume"
                onClicked: shellBackend.resumeSync()
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width > 720 ? 3 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            StatusCard {
                Layout.fillWidth: true
                title: "Sync State"
                value: shellBackend.syncState
                description: shellBackend.statusMessage
            }

            StatusCard {
                Layout.fillWidth: true
                title: "Mount Path"
                value: shellBackend.mountPath
                description: "User-configurable mount root"
            }

            StatusCard {
                Layout.fillWidth: true
                title: "Cache"
                value: shellBackend.cacheUsageLabel
                description: "Daemon-reported cache usage"
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
                        text: "Free Up Space"
                        onClicked: shellBackend.freeUpSpace()
                    }
                }
            }
        }
    }
}
