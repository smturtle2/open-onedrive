import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    id: page
    title: qsTr("Activity")

    property var requestDisconnect: null
    property var requestExplorer: null
    property var requestSetup: null
    property var requestLogs: null

    function headerTitle() {
        switch (shellBackend.appState) {
        case "daemon-unavailable":
            return qsTr("The daemon needs attention")
        case "welcome":
            return qsTr("Setup is still the next step")
        case "connecting":
            return qsTr("The workspace is still coming online")
        case "recovery":
            return qsTr("Recovery work is in progress")
        default:
            return qsTr("Queue and sync health")
        }
    }

    function headerBody() {
        switch (shellBackend.appState) {
        case "daemon-unavailable":
            return qsTr("Keep Logs nearby, then restart the service before returning to Files.")
        case "welcome":
            return qsTr("Choose a visible folder and finish sign-in. Files becomes the main workspace after setup.")
        case "connecting":
            return qsTr("Sign-in, mounting, or file activity is still running.")
        case "recovery":
            return qsTr("Use Setup for repair actions and Logs for the recent daemon trail.")
        default:
            return qsTr("This page stays compact on purpose. It summarizes status while Files remains the dominant workspace.")
        }
    }

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

    function showBanner() {
        return !shellBackend.daemonReachable
                || shellBackend.needsRemoteRepair
                || shellBackend.connectionState === "Connecting"
                || shellBackend.connectionState === "Error"
                || shellBackend.mountState === "Error"
                || shellBackend.mountState === "Degraded"
                || shellBackend.syncState === "Paused"
                || shellBackend.syncState === "Error"
                || shellBackend.conflictCount > 0
    }

    ColumnLayout {
        width: Math.min(parent.width, 1040)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.smallSpacing
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Heading {
                Layout.fillWidth: true
                level: 1
                wrapMode: Text.WordWrap
                text: page.headerTitle()
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.neutralTextColor
                text: page.headerBody()
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.showBanner()
            showCloseButton: false
            type: page.bannerType()
            text: shellBackend.statusMessage
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width > 860 ? 4 : width > 560 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Visible folder")
                value: shellBackend.effectiveMountPath.length > 0
                       ? shellBackend.effectiveMountPath
                       : qsTr("Not set")
                description: qsTr("The path shown in the file manager and exposed through the mounted workspace.")
                accentColor: "#295c8a"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Queue")
                value: qsTr("%1 pending").arg(shellBackend.queueDepth)
                description: qsTr("%1 downloads · %2 uploads").arg(shellBackend.pendingDownloads).arg(shellBackend.pendingUploads)
                accentColor: shellBackend.syncState === "Syncing" || shellBackend.syncState === "Scanning"
                             ? "#d38a1b"
                             : "#295c8a"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Offline bytes")
                value: shellBackend.cacheUsageLabel
                description: qsTr("Hydrated content stays under %1 inside the visible root.").arg(shellBackend.backingDirName)
                accentColor: "#147a51"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Residency")
                value: qsTr("%1 kept local").arg(shellBackend.pinnedFileCount)
                description: qsTr("%1 conflicts · last sync %2").arg(shellBackend.conflictCount).arg(shellBackend.lastSyncLabel)
                accentColor: shellBackend.conflictCount > 0 ? "#b3261e" : "#8b6f00"
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width > 860 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            Frame {
                Layout.fillWidth: true
                padding: Kirigami.Units.largeSpacing

                ColumnLayout {
                    anchors.fill: parent
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
                            { "label": qsTr("Active transfers"), "value": qsTr("%1").arg(shellBackend.activeTransferCount) }
                        ]

                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true

                            Label {
                                text: modelData.label
                                color: Kirigami.Theme.neutralTextColor
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

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: Kirigami.Theme.neutralTextColor
                        text: shellBackend.lastSyncError.length > 0
                              ? shellBackend.lastSyncError
                              : shellBackend.statusMessage
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
                        text: qsTr("Shortcuts")
                        level: 3
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: Kirigami.Theme.neutralTextColor
                        text: qsTr("Use Files for residency work, Setup for recovery actions, and Logs when you need the recent daemon trail.")
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
                            text: qsTr("Open Setup")
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
        }
    }
}
