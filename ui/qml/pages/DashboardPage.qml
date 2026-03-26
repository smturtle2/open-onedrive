import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    id: page
    title: qsTr("Dashboard")

    property var requestDisconnect: null
    property var requestExplorer: null
    property var requestSetup: null
    property var requestLogs: null

    function statusTitle() {
        switch (shellBackend.appState) {
        case "daemon-unavailable":
            return qsTr("The background service is offline")
        case "welcome":
            return qsTr("Connect this device to OneDrive")
        case "connecting":
            return qsTr("The workspace is still preparing")
        case "recovery":
            return qsTr("Recovery work needs your attention")
        case "running":
            return qsTr("Your visible OneDrive folder is ready")
        default:
            return qsTr("The workspace is ready")
        }
    }

    function statusBody() {
        switch (shellBackend.appState) {
        case "daemon-unavailable":
            return qsTr("Bring the daemon back first, then return to Files.")
        case "welcome":
            return qsTr("Choose the visible folder in Settings, then finish the browser sign-in.")
        case "connecting":
            return qsTr("Connection, mount, or transfer work is still running in the background.")
        case "recovery":
            return qsTr("Use Settings for repair and Logs for the recent daemon trail.")
        case "running":
            return qsTr("Files is the main workspace for online-only visibility and keep/free actions.")
        default:
            return qsTr("Open Files to continue working with the visible OneDrive folder.")
        }
    }

    function actionText() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Open Logs")
        }
        if (shellBackend.appState === "welcome") {
            return qsTr("Open Settings")
        }
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair Remote")
        }
        if (shellBackend.canRetry) {
            return qsTr("Retry Filesystem")
        }
        if (shellBackend.canMount) {
            return qsTr("Start Filesystem")
        }
        return qsTr("Open Files")
    }

    function runAction() {
        if (shellBackend.appState === "daemon-unavailable") {
            requestLogs ? requestLogs() : undefined
            return
        }
        if (shellBackend.appState === "welcome") {
            requestSetup ? requestSetup() : undefined
            return
        }
        if (shellBackend.needsRemoteRepair) {
            shellBackend.repairRemote()
            return
        }
        if (shellBackend.canRetry) {
            shellBackend.retryMount()
            return
        }
        if (shellBackend.canMount) {
            shellBackend.mountRemote()
            return
        }
        requestExplorer ? requestExplorer() : undefined
    }

    function queueLabel() {
        if (shellBackend.queuedActionCount <= 0) {
            return qsTr("Clear")
        }
        return qsTr("%1 queued").arg(shellBackend.queuedActionCount)
    }

    function storageLabel() {
        return shellBackend.cacheUsageLabel
    }

    ColumnLayout {
        width: Math.min(parent.width, 860)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.smallSpacing
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: shellBackend.statusMessage.length > 0
            showCloseButton: false
            type: shellBackend.appState === "recovery" || shellBackend.appState === "daemon-unavailable"
                  ? Kirigami.MessageType.Warning
                  : Kirigami.MessageType.Information
            text: shellBackend.statusMessage
        }

        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing
            color: "#ffffff"
            border.width: 1
            border.color: Qt.rgba(11 / 255, 28 / 255, 45 / 255, 0.09)

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.largeSpacing

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Label {
                        text: qsTr("Current status")
                        color: "#627284"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 1.1
                        font.bold: true
                    }

                    Kirigami.Heading {
                        Layout.fillWidth: true
                        level: 1
                        wrapMode: Text.WordWrap
                        text: page.statusTitle()
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: "#627284"
                        text: page.statusBody()
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Button {
                        text: page.actionText()
                        highlighted: true
                        onClicked: page.runAction()
                    }

                    Button {
                        text: qsTr("Open Files")
                        icon.name: "folder-open"
                        enabled: shellBackend.daemonReachable && shellBackend.remoteConfigured
                        onClicked: requestExplorer ? requestExplorer() : undefined
                    }

                    Button {
                        text: qsTr("Open Logs")
                        icon.name: "view-list-text"
                        onClicked: requestLogs ? requestLogs() : undefined
                    }

                    Button {
                        text: qsTr("Open Folder")
                        icon.name: "document-open-folder"
                        enabled: shellBackend.mountState === "Running" && shellBackend.effectiveMountPath.length > 0
                        onClicked: shellBackend.openMountLocation()
                    }
                }
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width > 680 ? 4 : 2
            columnSpacing: Kirigami.Units.mediumSpacing
            rowSpacing: Kirigami.Units.mediumSpacing

            Repeater {
                model: [
                    { "label": qsTr("Connection"), "value": shellBackend.connectionStateLabel },
                    { "label": qsTr("Filesystem"), "value": shellBackend.mountStateLabel },
                    { "label": qsTr("Sync"), "value": shellBackend.syncStateLabel },
                    { "label": qsTr("Queue"), "value": page.queueLabel() }
                ]

                delegate: Rectangle {
                    required property var modelData
                    Layout.fillWidth: true
                    radius: Kirigami.Units.mediumSpacing
                    color: "#ffffff"
                    border.width: 1
                    border.color: Qt.rgba(11 / 255, 28 / 255, 45 / 255, 0.09)
                    implicitHeight: metricColumn.implicitHeight + Kirigami.Units.mediumSpacing * 2

                    ColumnLayout {
                        id: metricColumn
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.mediumSpacing
                        spacing: 2

                        Label {
                            text: modelData.label
                            color: "#627284"
                            font.bold: true
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: modelData.value
                            font.bold: true
                        }
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing
            color: "#ffffff"
            border.width: 1
            border.color: Qt.rgba(11 / 255, 28 / 255, 45 / 255, 0.09)

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.mediumSpacing

                Label {
                    text: qsTr("Visible folder")
                    color: "#627284"
                    font.bold: true
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: shellBackend.effectiveMountPath.length > 0
                          ? shellBackend.effectiveMountPath
                          : qsTr("Choose a folder in Settings.")
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: "#627284"
                    text: qsTr("Local data: %1").arg(page.storageLabel())
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Button {
                        text: qsTr("Open Settings")
                        icon.name: "settings-configure"
                        onClicked: requestSetup ? requestSetup() : undefined
                    }

                    Button {
                        text: qsTr("Disconnect")
                        icon.name: "network-disconnect"
                        visible: shellBackend.remoteConfigured
                        onClicked: requestDisconnect ? requestDisconnect() : undefined
                    }
                }
            }
        }
    }
}
