import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    title: "Initial Setup"

    ColumnLayout {
        width: Math.min(parent.width, 760)
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }

        Kirigami.Heading {
            text: "Connect your Microsoft account"
            level: 1
        }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: "This shell stays thin: it captures the Client ID and mount path, then the Rust daemon will own auth, sync, and the FUSE mount."
        }

        Frame {
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Label {
                    text: "Microsoft app Client ID"
                }

                TextField {
                    Layout.fillWidth: true
                    placeholderText: "00000000-0000-0000-0000-000000000000"
                    text: shellBackend.clientId
                    onTextEdited: shellBackend.clientId = text
                }

                Label {
                    text: "Mount directory"
                }

                TextField {
                    Layout.fillWidth: true
                    placeholderText: "/home/you/OneDrive"
                    text: shellBackend.mountPath
                    onTextEdited: shellBackend.mountPath = text
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: "The daemon validates the selected directory before mounting, and this shell opens the Microsoft sign-in flow through D-Bus."
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true

            Button {
                text: "Save Setup"
                icon.name: "dialog-ok"
                onClicked: shellBackend.completeSetup()
            }

            Button {
                text: "Open Mount Folder"
                icon.name: "document-open-folder"
                enabled: shellBackend.mountPath.length > 0
                onClicked: shellBackend.openMountLocation()
            }

            Button {
                text: "Refresh Status"
                icon.name: "view-refresh"
                onClicked: shellBackend.refreshStatus()
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
