import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    id: page
    title: qsTr("Dashboard")

    function stateColor() {
        if (shellBackend.mountState === "Mounted") {
            return "#1f7a4d"
        }
        if (shellBackend.mountState === "Mounting" || shellBackend.mountState === "Connecting") {
            return "#c77700"
        }
        if (shellBackend.mountState === "Error") {
            return "#b3261e"
        }
        return "#3a5a78"
    }

    ColumnLayout {
        width: Math.min(parent.width, 960)
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }

        RowLayout {
            Layout.fillWidth: true

            Frame {
                Layout.fillWidth: true
                padding: Kirigami.Units.largeSpacing

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    RowLayout {
                        Layout.fillWidth: true

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            Kirigami.Heading {
                                text: qsTr("OneDrive mount control")
                                level: 1
                            }

                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: shellBackend.statusMessage
                                color: Kirigami.Theme.neutralTextColor
                            }
                        }

                        Rectangle {
                            radius: 999
                            color: page.stateColor()
                            implicitHeight: badgeLabel.implicitHeight + Kirigami.Units.smallSpacing * 2
                            implicitWidth: badgeLabel.implicitWidth + Kirigami.Units.largeSpacing * 2

                            Label {
                                id: badgeLabel
                                anchors.centerIn: parent
                                text: shellBackend.mountStateLabel
                                color: "white"
                                font.bold: true
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true

                        Button {
                            text: qsTr("Refresh")
                            icon.name: "view-refresh"
                            onClicked: shellBackend.refreshStatus()
                        }

                        Button {
                            text: qsTr("Mount")
                            icon.name: "folder-cloud"
                            enabled: shellBackend.canMount
                            onClicked: shellBackend.mountRemote()
                        }

                        Button {
                            text: qsTr("Unmount")
                            icon.name: "media-eject"
                            enabled: shellBackend.canUnmount
                            onClicked: shellBackend.unmountRemote()
                        }

                        Button {
                            text: qsTr("Retry")
                            icon.name: "view-refresh"
                            enabled: shellBackend.canRetry
                            onClicked: shellBackend.retryMount()
                        }

                        Button {
                            text: qsTr("Open Folder")
                            icon.name: "document-open-folder"
                            enabled: shellBackend.effectiveMountPath.length > 0
                            onClicked: shellBackend.openMountLocation()
                        }

                        Button {
                            text: qsTr("Disconnect")
                            icon.name: "network-disconnect"
                            onClicked: shellBackend.disconnectRemote()
                        }
                    }
                }
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width > 720 ? 4 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Backend")
                value: "rclone"
                description: shellBackend.rcloneVersion.length > 0 ? shellBackend.rcloneVersion : qsTr("Version pending")
                accentColor: "#4f7cff"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Mount state")
                value: shellBackend.mountStateLabel
                description: shellBackend.statusMessage
                accentColor: page.stateColor()
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Mount path")
                value: shellBackend.effectiveMountPath
                description: qsTr("Host filesystem path exposed by rclone mount")
                accentColor: "#3a5a78"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Cache")
                value: shellBackend.cacheUsageLabel
                description: qsTr("App-owned rclone VFS cache usage")
                accentColor: "#6f8b42"
            }
        }

        MountPathEditor {
            helperText: qsTr("Path changes stay local until you trigger Connect, Mount, or Retry from this dashboard.")
        }

        Frame {
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: qsTr("Diagnostics")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: shellBackend.lastLogLine.length > 0
                          ? shellBackend.lastLogLine
                          : qsTr("Recent rclone output will appear here.")
                }
            }
        }
    }
}
