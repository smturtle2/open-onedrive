import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    id: page
    title: shellBackend.needsRemoteRepair
           ? qsTr("Repair Remote")
           : shellBackend.remoteConfigured
             ? qsTr("Setup and Recovery")
             : qsTr("Set Up")

    property var requestDisconnect: null

    function primaryActionText() {
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair Remote")
        }
        if (shellBackend.remoteConfigured) {
            return qsTr("Retry Filesystem")
        }
        return qsTr("Connect OneDrive")
    }

    function primaryActionIcon() {
        if (shellBackend.needsRemoteRepair) {
            return "tools-wizard"
        }
        if (shellBackend.remoteConfigured) {
            return "view-refresh"
        }
        return "network-connect"
    }

    function stageTitle() {
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair the saved OneDrive sign-in")
        }
        if (shellBackend.remoteConfigured) {
            return qsTr("Manage the visible folder and recovery actions")
        }
        return qsTr("Choose the visible folder and sign in with rclone")
    }

    function stageBody() {
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair replaces only the app-owned sign-in used by open-onedrive. Offline bytes and local file state stay on this device.")
        }
        if (shellBackend.remoteConfigured) {
            return qsTr("Use this page when you need to move the visible folder, restart the filesystem, or disconnect the device cleanly.")
        }
        return qsTr("The setup flow is short: choose an empty folder, start browser sign-in, then let the filesystem expose OneDrive as a normal folder.")
    }

    function runPrimaryAction() {
        if (shellBackend.needsRemoteRepair) {
            shellBackend.repairRemote()
            return
        }
        if (shellBackend.remoteConfigured) {
            shellBackend.retryMount()
            return
        }
        shellBackend.beginConnect()
    }

    Menu {
        id: actionMenu

        MenuItem {
            text: qsTr("Start filesystem")
            icon.name: "folder-cloud"
            visible: shellBackend.canMount
            onTriggered: shellBackend.mountRemote()
        }

        MenuItem {
            text: qsTr("Refresh status")
            icon.name: "view-refresh"
            onTriggered: shellBackend.refreshStatus()
        }

        MenuSeparator { }

        MenuItem {
            text: qsTr("Disconnect")
            icon.name: "network-disconnect"
            visible: shellBackend.remoteConfigured
            onTriggered: requestDisconnect ? requestDisconnect() : shellBackend.disconnectRemote()
        }
    }

    ColumnLayout {
        width: Math.min(parent.width, 920)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.smallSpacing
        }

        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing * 1.2
            color: "#102333"
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.08)

            gradient: Gradient {
                GradientStop { position: 0.0; color: "#17364d" }
                GradientStop { position: 0.6; color: "#102334" }
                GradientStop { position: 1.0; color: "#0c1824" }
            }

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing * 1.2
                spacing: Kirigami.Units.largeSpacing

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Label {
                        text: qsTr("Setup workspace")
                        color: "#b9cce0"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 1.2
                        font.bold: true
                    }

                    Kirigami.Heading {
                        Layout.fillWidth: true
                        level: 1
                        wrapMode: Text.WordWrap
                        color: "white"
                        text: page.stageTitle()
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: "#d1deeb"
                        text: page.stageBody()
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Rectangle {
                        radius: 999
                        color: Qt.rgba(1, 1, 1, 0.12)
                        implicitWidth: stepOne.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: stepOne.implicitHeight + Kirigami.Units.smallSpacing

                        Label {
                            id: stepOne
                            anchors.centerIn: parent
                            color: "#eef4fb"
                            text: qsTr("1. Choose folder")
                            font.bold: true
                        }
                    }

                    Rectangle {
                        radius: 999
                        color: Qt.rgba(1, 1, 1, 0.12)
                        implicitWidth: stepTwo.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: stepTwo.implicitHeight + Kirigami.Units.smallSpacing

                        Label {
                            id: stepTwo
                            anchors.centerIn: parent
                            color: "#eef4fb"
                            text: qsTr("2. Sign in")
                            font.bold: true
                        }
                    }

                    Rectangle {
                        radius: 999
                        color: Qt.rgba(1, 1, 1, 0.12)
                        implicitWidth: stepThree.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: stepThree.implicitHeight + Kirigami.Units.smallSpacing

                        Label {
                            id: stepThree
                            anchors.centerIn: parent
                            color: "#eef4fb"
                            text: qsTr("3. Start filesystem")
                            font.bold: true
                        }
                    }
                }

                Button {
                    text: page.primaryActionText()
                    icon.name: page.primaryActionIcon()
                    highlighted: true
                    enabled: shellBackend.daemonReachable && shellBackend.mountPath.length > 0
                    onClicked: page.runPrimaryAction()
                }
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: !shellBackend.daemonReachable
                  ? Kirigami.MessageType.Warning
                  : shellBackend.needsRemoteRepair
                    ? Kirigami.MessageType.Error
                    : Kirigami.MessageType.Information
            showCloseButton: false
            text: !shellBackend.daemonReachable
                  ? qsTr("The background service is offline. You can still review the folder path here and return to Logs while it comes back.")
                  : shellBackend.needsRemoteRepair
                    ? qsTr("Repair only refreshes the app-owned sign-in. It does not delete online files in OneDrive.")
                    : qsTr("open-onedrive keeps its own rclone profile for this app and leaves your regular rclone setup untouched.")
        }

        MountPathEditor {
            helperText: qsTr("Pick an empty folder that should appear as your normal OneDrive path on this device.")
        }

        Frame {
            Layout.fillWidth: true
            padding: Kirigami.Units.largeSpacing

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: qsTr("Connection details")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: shellBackend.customClientIdConfigured
                          ? qsTr("A custom Microsoft client ID is already configured for this device.")
                          : qsTr("The default setup uses rclone's Microsoft app. Advanced client changes stay in config.toml rather than the first-run UI.")
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("Closing the window keeps open-onedrive in the system tray, so you can return later without stopping the daemon.")
                }
            }
        }

        Frame {
            Layout.fillWidth: true
            padding: Kirigami.Units.largeSpacing

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: qsTr("Actions")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: shellBackend.remoteConfigured
                          ? qsTr("Keep recovery actions here and use Explorer for file residency changes.")
                          : qsTr("Set the visible folder first, then start the browser sign-in. The tray stays available after you close the window.")
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
                        text: qsTr("Open Folder")
                        icon.name: "document-open-folder"
                        visible: shellBackend.effectiveMountPath.length > 0
                        enabled: shellBackend.effectiveMountPath.length > 0
                        onClicked: shellBackend.openMountLocation()
                    }

                    Button {
                        text: qsTr("More actions")
                        icon.name: "overflow-menu"
                        onClicked: actionMenu.open()
                    }
                }
            }
        }
    }
}
