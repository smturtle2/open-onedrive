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

    function bannerType() {
        if (!shellBackend.daemonReachable
                || shellBackend.needsRemoteRepair
                || shellBackend.connectionState === "Error"
                || shellBackend.mountState === "Error"
                || shellBackend.syncState === "Error") {
            return Kirigami.MessageType.Error
        }
        if (shellBackend.mountState === "Degraded"
                || shellBackend.syncState === "Paused"
                || shellBackend.conflictCount > 0) {
            return Kirigami.MessageType.Warning
        }
        return Kirigami.MessageType.Information
    }

    function statusTitle() {
        switch (shellBackend.appState) {
        case "daemon-unavailable":
            return qsTr("The background service needs to come back first")
        case "welcome":
            return qsTr("This device still needs its OneDrive connection")
        case "connecting":
            return qsTr("Connection or file work is still in progress")
        case "recovery":
            return qsTr("Review recovery work before returning to Files")
        case "running":
            return qsTr("The visible OneDrive folder is ready")
        default:
            return qsTr("The workspace is ready for the next step")
        }
    }

    function statusBody() {
        switch (shellBackend.appState) {
        case "daemon-unavailable":
            return qsTr("Open Logs to inspect the daemon, then restart the user service before browsing files.")
        case "welcome":
            return qsTr("Choose the visible folder in Settings, finish the Microsoft sign-in, then start the filesystem.")
        case "connecting":
            return qsTr("Keep this page open if you want a compact status view while queue work and sign-in continue.")
        case "recovery":
            return qsTr("Settings and Logs stay closest to repair actions. Files will reflect the same daemon state once recovery is finished.")
        case "running":
            return qsTr("Browse online-only and local items from Files, then keep or release them without leaving the visible folder workflow.")
        default:
            return qsTr("Use this page as the short status summary, then continue in Files.")
        }
    }

    function nextStepTitle() {
        if (shellBackend.appState === "welcome") {
            return qsTr("Connect OneDrive")
        }
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Open Logs")
        }
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair the saved sign-in")
        }
        if (shellBackend.canRetry) {
            return qsTr("Retry the filesystem")
        }
        if (shellBackend.canMount) {
            return qsTr("Start the visible folder")
        }
        if (shellBackend.syncState === "Paused") {
            return qsTr("Resume sync")
        }
        return qsTr("Open Files")
    }

    function nextStepBody() {
        if (shellBackend.appState === "welcome") {
            return qsTr("Settings is the only place you need for the first run: choose the folder path and begin sign-in.")
        }
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("The main UI can stay open, but the daemon must respond again before Files can refresh.")
        }
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair rebuilds only the app-owned remote profile and keeps the local backing store plus path state.")
        }
        if (shellBackend.canRetry) {
            return qsTr("Retry remounts the visible folder after reviewing the recent daemon trail.")
        }
        if (shellBackend.canMount) {
            return qsTr("Start the filesystem to expose OneDrive as the normal visible folder on this device.")
        }
        if (shellBackend.syncState === "Paused") {
            return qsTr("On-demand opens still work while paused, but local writes stay queued until you resume sync.")
        }
        return qsTr("Files remains the main workspace for online-only visibility, residency changes, and quick actions.")
    }

    ColumnLayout {
        width: Math.min(parent.width, 900)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.smallSpacing
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: shellBackend.statusMessage.length > 0
            showCloseButton: false
            type: page.bannerType()
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
                        text: qsTr("Workspace status")
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

                GridLayout {
                    Layout.fillWidth: true
                    columns: width > 760 ? 4 : width > 500 ? 2 : 1
                    columnSpacing: Kirigami.Units.mediumSpacing
                    rowSpacing: Kirigami.Units.mediumSpacing

                    Repeater {
                        model: [
                            { "label": qsTr("Visible folder"), "value": shellBackend.effectiveMountPath.length > 0 ? shellBackend.effectiveMountPath : qsTr("Not set") },
                            { "label": qsTr("Queue"), "value": qsTr("%1 pending").arg(shellBackend.queueDepth) },
                            { "label": qsTr("Local bytes"), "value": shellBackend.cacheUsageLabel },
                            { "label": qsTr("Kept on device"), "value": qsTr("%1 item(s)").arg(shellBackend.pinnedFileCount) }
                        ]

                        delegate: Rectangle {
                            required property var modelData
                            Layout.fillWidth: true
                            radius: Kirigami.Units.mediumSpacing
                            color: "#f5f8fb"
                            border.width: 1
                            border.color: Qt.rgba(11 / 255, 28 / 255, 45 / 255, 0.08)
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

                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Button {
                        text: qsTr("Open Files")
                        icon.name: "folder-open"
                        highlighted: true
                        enabled: shellBackend.daemonReachable && shellBackend.remoteConfigured
                        onClicked: requestExplorer ? requestExplorer() : undefined
                    }

                    Button {
                        text: qsTr("Open Settings")
                        icon.name: "settings-configure"
                        onClicked: requestSetup ? requestSetup() : undefined
                    }

                    Button {
                        text: qsTr("Open Logs")
                        icon.name: "view-list-text"
                        onClicked: requestLogs ? requestLogs() : undefined
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

        GridLayout {
            Layout.fillWidth: true
            columns: width > 760 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

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

                    Kirigami.Heading {
                        text: qsTr("Current state")
                        level: 3
                    }

                    Repeater {
                        model: [
                            { "label": qsTr("Connection"), "value": shellBackend.connectionStateLabel },
                            { "label": qsTr("Filesystem"), "value": shellBackend.mountStateLabel },
                            { "label": qsTr("Sync"), "value": shellBackend.syncStateLabel },
                            { "label": qsTr("Last sync"), "value": shellBackend.lastSyncLabel },
                            { "label": qsTr("Active transfers"), "value": qsTr("%1").arg(shellBackend.activeTransferCount) }
                        ]

                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true

                            Label {
                                text: modelData.label
                                color: "#627284"
                            }

                            Item {
                                Layout.fillWidth: true
                            }

                            Label {
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

                    Kirigami.Heading {
                        text: qsTr("Next step")
                        level: 3
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: page.nextStepTitle()
                        font.bold: true
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: "#627284"
                        text: page.nextStepBody()
                    }

                    Button {
                        text: shellBackend.syncState === "Paused" ? qsTr("Resume Sync") : qsTr("Open Files")
                        icon.name: shellBackend.syncState === "Paused" ? "media-playback-start" : "folder-open"
                        enabled: shellBackend.syncState === "Paused"
                                 ? shellBackend.canResumeSync
                                 : shellBackend.daemonReachable && shellBackend.remoteConfigured
                        onClicked: {
                            if (shellBackend.syncState === "Paused") {
                                shellBackend.resumeSync()
                            } else if (requestExplorer) {
                                requestExplorer()
                            }
                        }
                    }
                }
            }
        }
    }
}
