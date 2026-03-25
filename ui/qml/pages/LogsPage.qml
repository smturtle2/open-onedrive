import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    title: qsTr("Logs")

    ColumnLayout {
        width: Math.min(parent.width, 960)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }

        RowLayout {
            Layout.fillWidth: true

            Kirigami.Heading {
                text: qsTr("Recent rclone logs")
                level: 1
            }

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: qsTr("Copy")
                icon.name: "edit-copy"
                enabled: shellBackend.recentLogs.length > 0
                onClicked: shellBackend.copyRecentLogsToClipboard()
            }

            Button {
                text: qsTr("Refresh")
                icon.name: "view-refresh"
                onClicked: shellBackend.refreshLogs()
            }
        }

        Frame {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("Stay on this page while you retry or remount. The dashboard remains available even when the mount is in an error state.")
                }

                Label {
                    Layout.fillWidth: true
                    visible: shellBackend.recentLogs.length === 0
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("No log lines yet. Start the sign-in flow or mount the remote to capture rclone output.")
                }

                ListView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: shellBackend.recentLogs.length > 0
                    clip: true
                    spacing: Kirigami.Units.smallSpacing
                    model: shellBackend.recentLogs

                    delegate: Frame {
                        required property string modelData
                        width: ListView.view.width
                        padding: Kirigami.Units.mediumSpacing

                        Label {
                            width: parent.width
                            wrapMode: Text.WrapAnywhere
                            text: modelData
                            font.family: "monospace"
                        }
                    }
                }
            }
        }
    }
}
