import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    title: "Logs"

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
                text: "Recent rclone logs"
                level: 1
            }

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: "Refresh"
                onClicked: shellBackend.refreshLogs()
            }
        }

        Frame {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ListView {
                anchors.fill: parent
                clip: true
                spacing: Kirigami.Units.smallSpacing
                model: shellBackend.recentLogs

                delegate: Label {
                    required property string modelData
                    width: ListView.view.width
                    wrapMode: Text.WrapAnywhere
                    text: modelData
                }
            }
        }
    }
}
