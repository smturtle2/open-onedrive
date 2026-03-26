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

    function stateColor() {
        if (shellBackend.mountState === "Running") {
            return "#1f7a4d"
        }
        if (shellBackend.mountState === "Starting" || shellBackend.connectionState === "Connecting") {
            return "#c77700"
        }
        if (shellBackend.mountState === "Error"
                || shellBackend.connectionState === "Error"
                || shellBackend.syncState === "Error"
                || shellBackend.conflictCount > 0) {
            return "#b3261e"
        }
        return "#295c8a"
    }

    function syncColor() {
        if (shellBackend.syncState === "Error") {
            return "#b3261e"
        }
        if (shellBackend.syncState === "Paused") {
            return "#8b6f00"
        }
        if (shellBackend.syncState === "Syncing" || shellBackend.syncState === "Scanning") {
            return "#3c73d4"
        }
        return "#295c8a"
    }

    function stageTitle() {
        switch (shellBackend.appState) {
        case "daemon-unavailable":
            return qsTr("Reconnect the background service")
        case "welcome":
            return qsTr("Connect OneDrive and choose a visible root")
        case "connecting":
            return qsTr("Finish the browser sign-in flow")
        case "recovery":
            return shellBackend.needsRemoteRepair
                    ? qsTr("Repair the OneDrive profile and reconnect")
                    : qsTr("Recover the filesystem and resume sync")
        default:
            return qsTr("Operate the OneDrive filesystem from one place")
        }
    }

    function stageBody() {
        switch (shellBackend.appState) {
        case "daemon-unavailable":
            return qsTr("The shell is open, but the background service is offline. Restart it, then return here without losing the recent logs.")
        case "welcome":
            return qsTr("Pick an empty folder for the visible OneDrive root, then finish the Microsoft sign-in in your browser.")
        case "connecting":
            return qsTr("Sign-in or startup work is still in progress. Keep this window open if you want live status, or switch to Logs while it finishes.")
        case "recovery":
            return shellBackend.needsRemoteRepair
                    ? qsTr("The saved OneDrive sign-in for this machine is stale. Repair Remote rebuilds it and keeps your local hydrated files in place.")
                    : qsTr("The account is connected, but the filesystem needs attention. Review the status, fix the root path if needed, then retry.")
        default:
            return qsTr("The visible root, tray, logs, and Explorer all reflect the same daemon state, so you can browse files and check runtime health without typing paths manually.")
        }
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

    function helperText() {
        if (!shellBackend.daemonReachable) {
            return qsTr("You can still edit the folder path here. Start the daemon before you apply it.")
        }
        if (!shellBackend.remoteConfigured) {
            return qsTr("Choose an empty folder before you start the browser sign-in flow.")
        }
        return qsTr("Path changes stay local until you connect again or restart the filesystem.")
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
        if (shellBackend.effectiveMountPath.length > 0) {
            return qsTr("Open Explorer")
        }
        return qsTr("Refresh")
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
        if (shellBackend.effectiveMountPath.length > 0) {
            return "folder-open"
        }
        return "view-refresh"
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
        if (shellBackend.effectiveMountPath.length > 0 && requestExplorer) {
            requestExplorer()
            return
        }
        shellBackend.refreshStatus()
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
                text: qsTr("Disconnect removes the app-owned rclone profile, clears the hydrated backing store, and resets local path state for this device.")
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.neutralTextColor
                text: qsTr("Use this only when you want to sign in again or intentionally remove the local OneDrive setup.")
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
            text: qsTr("Rescan remote")
            icon.name: "folder-sync"
            visible: shellBackend.daemonReachable && shellBackend.remoteConfigured
            onTriggered: shellBackend.rescanRemote()
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
            text: qsTr("Open root folder")
            icon.name: "document-open-folder"
            visible: shellBackend.effectiveMountPath.length > 0
            onTriggered: shellBackend.openMountLocation()
        }

        MenuItem {
            text: qsTr("Disconnect")
            icon.name: "network-disconnect"
            visible: shellBackend.remoteConfigured
            onTriggered: requestDisconnect ? requestDisconnect() : disconnectDialog.open()
        }
    }

    ColumnLayout {
        width: Math.min(parent.width, 980)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }

        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing
            color: "#0d2230"
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.08)

            gradient: Gradient {
                GradientStop { position: 0.0; color: "#143449" }
                GradientStop { position: 0.62; color: "#102732" }
                GradientStop { position: 1.0; color: "#0b161d" }
            }

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing * 1.3
                spacing: Kirigami.Units.largeSpacing

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Label {
                        text: qsTr("open-onedrive")
                        color: "#c8d9e5"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 1.6
                        font.bold: true
                    }

                    Kirigami.Heading {
                        Layout.fillWidth: true
                        level: 1
                        text: page.stageTitle()
                        color: "white"
                        wrapMode: Text.WordWrap
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: "#c4d3dc"
                        text: page.stageBody()
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Rectangle {
                        radius: 999
                        color: Qt.rgba(1, 1, 1, 0.1)
                        implicitWidth: connectionRow.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: connectionRow.implicitHeight + Kirigami.Units.smallSpacing

                        RowLayout {
                            id: connectionRow
                            anchors.centerIn: parent
                            spacing: Kirigami.Units.smallSpacing

                            Label {
                                text: qsTr("Connection")
                                color: "#b3c6d3"
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
                        color: Qt.rgba(1, 1, 1, 0.1)
                        implicitWidth: filesystemRow.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: filesystemRow.implicitHeight + Kirigami.Units.smallSpacing

                        RowLayout {
                            id: filesystemRow
                            anchors.centerIn: parent
                            spacing: Kirigami.Units.smallSpacing

                            Label {
                                text: qsTr("Filesystem")
                                color: "#b3c6d3"
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
                        color: Qt.rgba(1, 1, 1, 0.1)
                        implicitWidth: syncRow.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: syncRow.implicitHeight + Kirigami.Units.smallSpacing

                        RowLayout {
                            id: syncRow
                            anchors.centerIn: parent
                            spacing: Kirigami.Units.smallSpacing

                            Label {
                                text: qsTr("Sync")
                                color: "#b3c6d3"
                            }

                            Label {
                                text: shellBackend.syncStateLabel
                                color: "white"
                                font.bold: true
                            }
                        }
                    }

                    Rectangle {
                        radius: 999
                        color: Qt.rgba(1, 1, 1, 0.1)
                        implicitWidth: rootRow.implicitWidth + Kirigami.Units.largeSpacing
                        implicitHeight: rootRow.implicitHeight + Kirigami.Units.smallSpacing

                        RowLayout {
                            id: rootRow
                            anchors.centerIn: parent
                            spacing: Kirigami.Units.smallSpacing

                            Label {
                                text: qsTr("Root")
                                color: "#b3c6d3"
                            }

                            Label {
                                text: shellBackend.effectiveMountPath.length > 0
                                      ? shellBackend.effectiveMountPath
                                      : qsTr("Not set")
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
                                 && (!shellBackend.remoteConfigured || true)
                        onClicked: page.runPrimaryAction()
                    }

                    Button {
                        text: qsTr("Refresh Logs")
                        icon.name: "view-list-text"
                        onClicked: shellBackend.refreshLogs()
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

        Frame {
            Layout.fillWidth: true
            visible: !shellBackend.daemonReachable
            padding: Kirigami.Units.largeSpacing

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: qsTr("Background service unavailable")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: qsTr("Start `openonedrived` once through the launcher, or run `systemctl --user start openonedrived.service`, then refresh status here. The Logs tab stays available even while the daemon is down.")
                    color: Kirigami.Theme.neutralTextColor
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
                    text: shellBackend.remoteConfigured ? qsTr("Root folder and connection") : qsTr("First-run setup")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: shellBackend.remoteConfigured
                          ? qsTr("Change the visible root here when you need to move OneDrive on this device. open-onedrive carries the hidden local cache to the new root before restart when possible.")
                          : qsTr("Choose where the visible OneDrive root should live on this machine. The folder must be empty except for the hidden local cache folder managed by the daemon.")
                }

                MountPathEditor {
                    helperText: page.helperText()
                }

                Label {
                    Layout.fillWidth: true
                    visible: !shellBackend.remoteConfigured
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("1. Choose an empty folder. 2. Start sign-in. 3. Start the filesystem when the account is ready.")
                }
            }
        }

        Frame {
            Layout.fillWidth: true
            visible: shellBackend.remoteConfigured
            padding: Kirigami.Units.largeSpacing

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: qsTr("Runtime inspector")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("A compact view of the runtime so you can judge queue health, local cache size, and recovery status at a glance.")
                }

                GridLayout {
                    Layout.fillWidth: true
                    columns: width > 860 ? 4 : width > 540 ? 2 : 1
                    columnSpacing: Kirigami.Units.largeSpacing
                    rowSpacing: Kirigami.Units.mediumSpacing

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Label {
                            text: qsTr("Backend")
                            color: Kirigami.Theme.neutralTextColor
                        }

                        Kirigami.Heading {
                            text: shellBackend.rcloneVersion.length > 0 ? shellBackend.rcloneVersion : qsTr("Pending")
                            level: 2
                            wrapMode: Text.Wrap
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Kirigami.Theme.disabledTextColor
                            text: qsTr("Custom FUSE plus rclone primitives for listing and transfers")
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Label {
                            text: qsTr("Sync queue")
                            color: Kirigami.Theme.neutralTextColor
                        }

                        Kirigami.Heading {
                            text: qsTr("%1 total").arg(shellBackend.queueDepth)
                            level: 2
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Kirigami.Theme.disabledTextColor
                            text: qsTr("%1 downloads pending · %2 uploads pending").arg(shellBackend.pendingDownloads).arg(shellBackend.pendingUploads)
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Label {
                            text: qsTr("Backing store")
                            color: Kirigami.Theme.neutralTextColor
                        }

                        Kirigami.Heading {
                            text: shellBackend.cacheUsageLabel
                            level: 2
                            wrapMode: Text.Wrap
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Kirigami.Theme.disabledTextColor
                            text: qsTr("Hydrated bytes live in the hidden %1 folder").arg(shellBackend.backingDirName)
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Label {
                            text: qsTr("Residency")
                            color: Kirigami.Theme.neutralTextColor
                        }

                        Kirigami.Heading {
                            text: qsTr("%1 pinned").arg(shellBackend.pinnedFileCount)
                            level: 2
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: Kirigami.Theme.disabledTextColor
                            text: qsTr("%1 conflicts · last sync %2").arg(shellBackend.conflictCount).arg(shellBackend.lastSyncLabel)
                        }
                    }
                }
            }
        }

        Frame {
            Layout.fillWidth: true
            visible: shellBackend.remoteConfigured
            padding: Kirigami.Units.largeSpacing

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: qsTr("Explorer")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("Browse folders, search paths, and switch files between kept-local and online-only without manually entering paths.")
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Button {
                        text: qsTr("Open Explorer")
                        icon.name: "folder-open"
                        highlighted: true
                        enabled: shellBackend.daemonReachable
                        onClicked: requestExplorer ? requestExplorer() : undefined
                    }

                    Button {
                        text: qsTr("Open Root Folder")
                        icon.name: "document-open-folder"
                        enabled: shellBackend.effectiveMountPath.length > 0
                        onClicked: shellBackend.openMountLocation()
                    }
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
                    text: qsTr("Diagnostics")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: shellBackend.lastLogLine.length > 0
                          ? shellBackend.lastLogLine
                          : qsTr("Recent daemon and rclone output will appear here.")
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: shellBackend.remoteConfigured
                          ? qsTr("Last sync: %1").arg(shellBackend.lastSyncLabel)
                          : qsTr("No remote profile yet. Start with the root folder and browser sign-in above.")
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
                    text: qsTr("Dolphin overlays and actions operate on the visible root only and ignore the hidden backing directory. Use Explorer for residency work and Logs for full daemon output.")
                }
            }
        }
    }
}
