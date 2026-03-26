import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    id: page
    title: qsTr("Overview")

    property var requestDisconnect: null
    property var requestExplorer: null
    property var requestSetup: null
    property var requestLogs: null

    function stageTitle() {
        switch (shellBackend.appState) {
        case "daemon-unavailable":
            return qsTr("Bring the background service back online")
        case "welcome":
            return qsTr("Connect OneDrive and prepare the visible folder")
        case "connecting":
            return qsTr("Finish sign-in and let the local folder come up")
        case "recovery":
            return shellBackend.needsRemoteRepair
                    ? qsTr("Repair the saved sign-in and reconnect")
                    : qsTr("Recover the filesystem and resume sync")
        default:
            return qsTr("Operate the visible OneDrive folder from one place")
        }
    }

    function stageBody() {
        switch (shellBackend.appState) {
        case "daemon-unavailable":
            return qsTr("Logs stay available here, so you can restart the service without losing recovery context.")
        case "welcome":
            return qsTr("The setup flow is short: choose an empty folder, sign in through your browser, then start the filesystem.")
        case "connecting":
            return qsTr("Sign-in, startup, or transfer work is still running. Keep this page nearby for status, then switch to Explorer when the folder is ready.")
        case "recovery":
            return shellBackend.needsRemoteRepair
                    ? qsTr("Repair Remote rebuilds only the app-owned OneDrive sign-in and keeps offline bytes on this device.")
                    : qsTr("The account is connected, but something needs attention before normal sync resumes.")
        default:
            return qsTr("Explorer, tray controls, Dolphin actions, and logs all reflect the same daemon state.")
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

    function primaryActionText() {
        if (!shellBackend.remoteConfigured) {
            return qsTr("Connect OneDrive")
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
        return qsTr("Open Explorer")
    }

    function primaryActionIcon() {
        if (!shellBackend.remoteConfigured) {
            return "network-connect"
        }
        if (shellBackend.needsRemoteRepair) {
            return "tools-wizard"
        }
        if (shellBackend.canRetry) {
            return "view-refresh"
        }
        if (shellBackend.canMount) {
            return "folder-cloud"
        }
        return "folder-open"
    }

    function runPrimaryAction() {
        if (!shellBackend.remoteConfigured) {
            shellBackend.beginConnect()
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
        if (requestExplorer) {
            requestExplorer()
            return
        }
        shellBackend.openMountLocation()
    }

    Dialog {
        id: disconnectDialog
        modal: true
        title: qsTr("Disconnect OneDrive")
        standardButtons: Dialog.Cancel | Dialog.Ok

        onAccepted: shellBackend.disconnectRemote()

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Disconnect removes the app-owned sign-in, clears offline bytes, and resets local file state for this device.")
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.neutralTextColor
                text: qsTr("Use this only when you want to sign in again or intentionally remove the local setup.")
            }
        }
    }

    ColumnLayout {
        width: Math.min(parent.width, 1060)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.smallSpacing
        }

        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing * 1.2
            color: "#0e2031"
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.08)

            gradient: Gradient {
                GradientStop { position: 0.0; color: "#16344b" }
                GradientStop { position: 0.55; color: "#102334" }
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
                        text: qsTr("Current workspace")
                        color: "#b4c8dd"
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
                        color: "#d0deed"
                        text: page.stageBody()
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Rectangle {
                        radius: 999
                        color: Qt.rgba(1, 1, 1, 0.12)
                        implicitWidth: connectionRow.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: connectionRow.implicitHeight + Kirigami.Units.smallSpacing

                        RowLayout {
                            id: connectionRow
                            anchors.centerIn: parent
                            spacing: Kirigami.Units.smallSpacing

                            Label {
                                text: qsTr("Connection")
                                color: "#b4c8dd"
                            }

                            Label {
                                text: shellBackend.connectionStateLabel
                                color: "white"
                                font.bold: true
                            }
                        }
                    }

                    Rectangle {
                        radius: 999
                        color: Qt.rgba(1, 1, 1, 0.12)
                        implicitWidth: filesystemRow.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: filesystemRow.implicitHeight + Kirigami.Units.smallSpacing

                        RowLayout {
                            id: filesystemRow
                            anchors.centerIn: parent
                            spacing: Kirigami.Units.smallSpacing

                            Label {
                                text: qsTr("Filesystem")
                                color: "#b4c8dd"
                            }

                            Label {
                                text: shellBackend.mountStateLabel
                                color: "white"
                                font.bold: true
                            }
                        }
                    }

                    Rectangle {
                        radius: 999
                        color: Qt.rgba(1, 1, 1, 0.12)
                        implicitWidth: syncRow.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: syncRow.implicitHeight + Kirigami.Units.smallSpacing

                        RowLayout {
                            id: syncRow
                            anchors.centerIn: parent
                            spacing: Kirigami.Units.smallSpacing

                            Label {
                                text: qsTr("Sync")
                                color: "#b4c8dd"
                            }

                            Label {
                                text: shellBackend.syncStateLabel
                                color: "white"
                                font.bold: true
                            }
                        }
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Button {
                        text: page.primaryActionText()
                        icon.name: page.primaryActionIcon()
                        highlighted: true
                        enabled: shellBackend.daemonReachable
                                 && (!shellBackend.needsRemoteRepair || shellBackend.mountPath.length > 0)
                        onClicked: page.runPrimaryAction()
                    }

                    Button {
                        text: qsTr("Open Setup")
                        icon.name: "settings-configure"
                        onClicked: requestSetup ? requestSetup() : undefined
                    }

                    Button {
                        text: qsTr("Open Logs")
                        icon.name: "view-list-text"
                        onClicked: requestLogs ? requestLogs() : shellBackend.refreshLogs()
                    }

                    Button {
                        text: qsTr("More actions")
                        icon.name: "overflow-menu"
                        onClicked: overflowMenu.open()
                    }
                }
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
            columns: width > 920 ? 4 : width > 620 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Visible folder")
                value: shellBackend.effectiveMountPath.length > 0
                       ? shellBackend.effectiveMountPath
                       : qsTr("Not set")
                description: qsTr("The normal folder path that apps, Dolphin, and the tray all operate on.")
                accentColor: "#295c8a"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Sync queue")
                value: qsTr("%1 total").arg(shellBackend.queueDepth)
                description: qsTr("%1 downloads pending · %2 uploads pending").arg(shellBackend.pendingDownloads).arg(shellBackend.pendingUploads)
                accentColor: shellBackend.syncState === "Syncing" || shellBackend.syncState === "Scanning"
                             ? "#3c73d4"
                             : "#295c8a"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Offline bytes")
                value: shellBackend.cacheUsageLabel
                description: qsTr("Offline file contents stay in the hidden %1 folder inside the visible root.").arg(shellBackend.backingDirName)
                accentColor: "#1f7a4d"
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
            columns: width > 840 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            Frame {
                Layout.fillWidth: true
                Layout.fillHeight: true
                padding: Kirigami.Units.largeSpacing

                ColumnLayout {
                    anchors.fill: parent
                    spacing: Kirigami.Units.mediumSpacing

                    Kirigami.Heading {
                        text: qsTr("Next steps")
                        level: 3
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: Kirigami.Theme.neutralTextColor
                        text: !shellBackend.remoteConfigured
                              ? qsTr("Setup owns the folder path and sign-in flow. Once that is ready, Explorer becomes the fastest place to work with files.")
                              : qsTr("Use Explorer for residency changes, Setup for folder moves and reconnect, and Logs when recovery work needs details.")
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Button {
                            text: qsTr("Open Explorer")
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
                            text: qsTr("Open Folder")
                            icon.name: "document-open-folder"
                            enabled: shellBackend.mountState === "Running" && shellBackend.effectiveMountPath.length > 0
                            onClicked: shellBackend.openMountLocation()
                        }
                    }
                }
            }

            Frame {
                Layout.fillWidth: true
                Layout.fillHeight: true
                padding: Kirigami.Units.largeSpacing

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
                        text: shellBackend.lastLogLine.length > 0
                              ? shellBackend.lastLogLine
                              : qsTr("Recent daemon and rclone activity will appear here.")
                    }

                    Label {
                        Layout.fillWidth: true
                        visible: shellBackend.lastSyncError.length > 0
                        wrapMode: Text.WordWrap
                        color: Kirigami.Theme.negativeTextColor
                        text: shellBackend.lastSyncError
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: Kirigami.Theme.neutralTextColor
                        text: qsTr("Closing the window keeps the tray controls alive. Use Logs for the full recovery trail when something needs attention.")
                    }

                    Button {
                        text: qsTr("Open Logs")
                        icon.name: "view-list-text"
                        onClicked: requestLogs ? requestLogs() : undefined
                    }
                }
            }
        }

        Menu {
            id: overflowMenu

            MenuItem {
                text: qsTr("Refresh status")
                icon.name: "view-refresh"
                onTriggered: shellBackend.refreshStatus()
            }

            MenuItem {
                text: qsTr("Start filesystem")
                icon.name: "folder-cloud"
                visible: shellBackend.canMount
                onTriggered: shellBackend.mountRemote()
            }

            MenuItem {
                text: qsTr("Stop filesystem")
                icon.name: "media-eject"
                visible: shellBackend.canUnmount
                onTriggered: shellBackend.unmountRemote()
            }

            MenuItem {
                text: qsTr("Retry filesystem")
                icon.name: "view-refresh"
                visible: shellBackend.canRetry
                onTriggered: shellBackend.retryMount()
            }

            MenuItem {
                text: qsTr("Pause sync")
                icon.name: "media-playback-pause"
                visible: shellBackend.canPauseSync
                onTriggered: shellBackend.pauseSync()
            }

            MenuItem {
                text: qsTr("Resume sync")
                icon.name: "media-playback-start"
                visible: shellBackend.canResumeSync
                onTriggered: shellBackend.resumeSync()
            }

            MenuSeparator { }

            MenuItem {
                text: qsTr("Disconnect")
                icon.name: "network-disconnect"
                visible: shellBackend.remoteConfigured
                onTriggered: requestDisconnect ? requestDisconnect() : disconnectDialog.open()
            }
        }
    }
}
