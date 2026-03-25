import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    id: page
    title: "Sign In"

    property bool showAdvancedClientId: !shellBackend.clientIdConfigured

    ColumnLayout {
        width: Math.min(parent.width, 760)
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }

        Kirigami.Heading {
            text: "Sign in to OneDrive"
            level: 1
        }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: "Choose where OneDrive should appear on this machine, then start the Microsoft sign-in flow."
        }

        Frame {
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

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
                    text: "The daemon validates the selected directory before mounting and keeps the OneDrive view there."
                }
            }
        }

        Frame {
            Layout.fillWidth: true

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                RowLayout {
                    Layout.fillWidth: true

                    Kirigami.Heading {
                        text: "Microsoft Sign-In"
                        level: 3
                    }

                    Item {
                        Layout.fillWidth: true
                    }

                    Button {
                        text: page.showAdvancedClientId ? "Hide Advanced" : "Advanced"
                        onClicked: page.showAdvancedClientId = !page.showAdvancedClientId
                    }
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: shellBackend.clientIdConfigured
                          ? "A Microsoft OAuth client is already configured. You can sign in directly."
                          : "This build does not ship a bundled Microsoft OAuth client yet. Add your Client ID below once, then sign in."
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: page.showAdvancedClientId || !shellBackend.clientIdConfigured

                    Label {
                        text: shellBackend.clientIdConfigured
                              ? "Optional custom Microsoft OAuth Client ID"
                              : "Microsoft OAuth Client ID"
                    }

                    TextField {
                        Layout.fillWidth: true
                        placeholderText: "00000000-0000-0000-0000-000000000000"
                        text: shellBackend.clientId
                        onTextEdited: shellBackend.clientId = text
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: shellBackend.clientIdConfigured
                              ? "Leave this blank to keep using the configured client. Fill it only if you want to override it."
                              : "You can also provide the same value through OPEN_ONEDRIVE_CLIENT_ID before launching the app."
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true

            Button {
                text: shellBackend.authPending ? "Waiting for Browser" : "Sign in with Microsoft"
                icon.name: "im-user-online"
                enabled: shellBackend.mountPath.length > 0
                         && !shellBackend.authPending
                         && (shellBackend.clientIdConfigured || shellBackend.clientId.length > 0)
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
            visible: !shellBackend.clientIdConfigured && shellBackend.clientId.length === 0
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.neutralTextColor
            text: "Sign-in is disabled until a Microsoft OAuth Client ID is configured."
        }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.neutralTextColor
            text: shellBackend.statusMessage
        }
    }
}
