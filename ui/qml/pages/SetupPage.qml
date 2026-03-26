import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    id: page
    title: qsTr("Settings")

    property var requestDisconnect: null
    property bool ready: false

    readonly property color canvasColor: "#edf2f5"
    readonly property color surfaceColor: "#ffffff"
    readonly property color mutedSurfaceColor: "#f5f7fa"
    readonly property color lineColor: Qt.rgba(14 / 255, 27 / 255, 42 / 255, 0.09)
    readonly property color headingColor: "#112334"
    readonly property color mutedTextColor: "#617182"

    function stateAccent() {
        if (shellBackend.appState === "running") {
            return "#1f7a4d"
        }
        if (shellBackend.appState === "connecting") {
            return "#bf7a1d"
        }
        if (shellBackend.appState === "recovery" || shellBackend.appState === "daemon-unavailable") {
            return "#b53b2d"
        }
        return "#245f92"
    }

    function stageTitle() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Background service unavailable")
        }
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Remote profile needs repair")
        }
        if (!shellBackend.remoteConfigured) {
            return qsTr("Choose the visible folder and connect")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Connection or sync work is still running")
        }
        if (shellBackend.mountState === "Running") {
            return qsTr("Visible folder is ready")
        }
        return qsTr("Visible folder and service controls")
    }

    function stageBody() {
        if (shellBackend.appState === "daemon-unavailable") {
            return qsTr("Start openonedrived, then return here to manage the visible OneDrive folder.")
        }
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair rebuilds only the app-owned OneDrive profile and keeps local hydrated bytes on this device.")
        }
        if (!shellBackend.remoteConfigured) {
            return qsTr("This window stays intentionally small: choose the folder, connect the account, then work from your file manager.")
        }
        if (shellBackend.appState === "connecting") {
            return qsTr("Queue, mount, or transfer work is still active in the daemon.")
        }
        return qsTr("Use Dolphin or Nautilus for file residency actions. This app only keeps the folder path and daemon state in shape.")
    }

    function primaryActionText() {
        if (shellBackend.needsRemoteRepair) {
            return qsTr("Repair remote")
        }
        if (!shellBackend.remoteConfigured) {
            return qsTr("Connect OneDrive")
        }
        return qsTr("Reconnect")
    }

    function primaryActionIcon() {
        if (shellBackend.needsRemoteRepair) {
            return "tools-wizard"
        }
        return "network-connect"
    }

    function runPrimaryAction() {
        if (shellBackend.needsRemoteRepair) {
            shellBackend.repairRemote()
            return
        }
        shellBackend.beginConnect()
    }

    function syncActionText() {
        return shellBackend.canResumeSync ? qsTr("Resume sync") : qsTr("Pause sync")
    }

    function syncActionIcon() {
        return shellBackend.canResumeSync ? "media-playback-start" : "media-playback-pause"
    }

    function runSyncAction() {
        if (shellBackend.canResumeSync) {
            shellBackend.resumeSync()
        } else {
            shellBackend.pauseSync()
        }
    }

    function queueSummary() {
        if (shellBackend.activeActionKind.length > 0) {
            return qsTr("%1 active · %2 queued").arg(shellBackend.activeActionKind).arg(shellBackend.queuedActionCount)
        }
        if (shellBackend.queuedActionCount > 0) {
            return qsTr("%1 queued").arg(shellBackend.queuedActionCount)
        }
        return qsTr("No queued work")
    }

    function statusRows() {
        return [
            { "label": qsTr("Connection"), "value": shellBackend.connectionStateLabel },
            { "label": qsTr("Filesystem"), "value": shellBackend.mountStateLabel },
            { "label": qsTr("Sync"), "value": shellBackend.syncStateLabel },
            { "label": qsTr("Queue"), "value": page.queueSummary() }
        ]
    }

    function diagnosticsRows() {
        return [
            { "label": qsTr("Last sync"), "value": shellBackend.lastSyncLabel },
            { "label": qsTr("Pinned files"), "value": qsTr("%1").arg(shellBackend.pinnedFileCount) },
            { "label": qsTr("Backing usage"), "value": shellBackend.cacheUsageLabel },
            { "label": qsTr("rclone"), "value": shellBackend.rcloneVersion.length > 0 ? shellBackend.rcloneVersion : qsTr("Not detected yet") }
        ]
    }

    Dialog {
        id: disconnectDialog
        title: qsTr("Disconnect OneDrive")
        modal: true
        standardButtons: Dialog.Cancel | Dialog.Ok

        onAccepted: shellBackend.disconnectRemote()

        contentItem: ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: qsTr("Disconnect removes the app-owned sign-in and local sync state. Files stored in OneDrive stay online.")
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: page.mutedTextColor
                text: qsTr("Use this only when detaching the device or rebuilding the local profile.")
            }
        }
    }

    Component.onCompleted: ready = true

    background: Rectangle {
        color: page.canvasColor
    }

    ColumnLayout {
        width: Math.min(parent.width, 940)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.smallSpacing
        }

        Rectangle {
            id: heroCard
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing * 1.2
            color: page.surfaceColor
            border.width: 1
            border.color: page.lineColor
            opacity: page.ready ? 1 : 0
            implicitHeight: heroContent.implicitHeight + Kirigami.Units.largeSpacing * 2

            Behavior on opacity {
                NumberAnimation { duration: 160 }
            }

            ColumnLayout {
                id: heroContent
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Label {
                            text: qsTr("OPEN-ONEDRIVE")
                            color: page.mutedTextColor
                            font.capitalization: Font.AllUppercase
                            font.letterSpacing: 1.4
                            font.bold: true
                        }

                        Kirigami.Heading {
                            Layout.fillWidth: true
                            level: 1
                            wrapMode: Text.WordWrap
                            color: page.headingColor
                            text: page.stageTitle()
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: page.mutedTextColor
                            text: page.stageBody()
                        }
                    }

                    ColumnLayout {
                        spacing: Kirigami.Units.mediumSpacing

                        Rectangle {
                            radius: 999
                            color: Qt.rgba(page.stateAccent().r, page.stateAccent().g, page.stateAccent().b, 0.12)
                            border.width: 1
                            border.color: Qt.rgba(page.stateAccent().r, page.stateAccent().g, page.stateAccent().b, 0.24)
                            implicitWidth: statusBadge.implicitWidth + Kirigami.Units.largeSpacing
                            implicitHeight: statusBadge.implicitHeight + Kirigami.Units.smallSpacing * 2

                            Label {
                                id: statusBadge
                                anchors.centerIn: parent
                                color: page.stateAccent()
                                font.bold: true
                                text: shellBackend.daemonReachable ? shellBackend.syncStateLabel : qsTr("Offline")
                            }
                        }

                        Button {
                            text: page.primaryActionText()
                            icon.name: page.primaryActionIcon()
                            highlighted: true
                            enabled: shellBackend.daemonReachable && shellBackend.mountPathValid
                            onClicked: page.runPrimaryAction()
                        }
                    }
                }

                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    showCloseButton: false
                    visible: shellBackend.statusMessage.length > 0
                    type: shellBackend.appState === "recovery" || shellBackend.appState === "daemon-unavailable"
                          ? Kirigami.MessageType.Warning
                          : Kirigami.MessageType.Information
                    text: shellBackend.statusMessage
                }
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width > 820 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            Rectangle {
                id: statusCard
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                radius: Kirigami.Units.largeSpacing
                color: page.surfaceColor
                border.width: 1
                border.color: page.lineColor
                opacity: page.ready ? 1 : 0
                implicitHeight: statusCardContent.implicitHeight + Kirigami.Units.largeSpacing * 2

                Behavior on opacity {
                    NumberAnimation { duration: 180 }
                }

                ColumnLayout {
                    id: statusCardContent
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.mediumSpacing

                    Label {
                        text: qsTr("Service status")
                        color: page.mutedTextColor
                        font.bold: true
                    }

                    Repeater {
                        model: page.statusRows()

                        delegate: RowLayout {
                            required property var modelData
                            Layout.fillWidth: true

                            Label {
                                text: modelData.label
                                color: page.mutedTextColor
                            }

                            Item {
                                Layout.fillWidth: true
                            }

                            Label {
                                text: modelData.value
                                font.bold: true
                                horizontalAlignment: Text.AlignRight
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        radius: Kirigami.Units.mediumSpacing
                        color: page.mutedSurfaceColor
                        border.width: 1
                        border.color: page.lineColor
                        implicitHeight: latestLogContent.implicitHeight + Kirigami.Units.mediumSpacing * 2

                        ColumnLayout {
                            id: latestLogContent
                            anchors.fill: parent
                            anchors.margins: Kirigami.Units.mediumSpacing
                            spacing: Kirigami.Units.smallSpacing

                            Label {
                                text: qsTr("Latest daemon line")
                                color: page.mutedTextColor
                                font.bold: true
                            }

                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: shellBackend.lastLogLine.length > 0
                                      ? shellBackend.lastLogLine
                                      : qsTr("No recent log line yet.")
                            }
                        }
                    }
                }
            }

            Rectangle {
                id: folderCard
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                radius: Kirigami.Units.largeSpacing
                color: page.surfaceColor
                border.width: 1
                border.color: page.lineColor
                opacity: page.ready ? 1 : 0
                implicitHeight: folderCardContent.implicitHeight + Kirigami.Units.largeSpacing * 2

                Behavior on opacity {
                    NumberAnimation { duration: 220 }
                }

                ColumnLayout {
                    id: folderCardContent
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.mediumSpacing

                    Label {
                        text: qsTr("Visible folder")
                        color: page.mutedTextColor
                        font.bold: true
                    }

                    MountPathEditor {
                        helperText: qsTr("Choose an empty directory the first time you set up the visible OneDrive root. Explorer overlays and residency actions appear here after connection.")
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Button {
                            text: qsTr("Open folder")
                            icon.name: "document-open-folder"
                            enabled: shellBackend.mountState === "Running" && shellBackend.effectiveMountPath.length > 0
                            onClicked: shellBackend.openMountLocation()
                        }

                        Button {
                            text: qsTr("Refresh status")
                            icon.name: "view-refresh"
                            enabled: shellBackend.daemonReachable
                            onClicked: {
                                shellBackend.refreshStatus()
                                shellBackend.refreshLogs()
                            }
                        }
                    }
                }
            }
        }

        Rectangle {
            id: actionsCard
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing
            color: page.surfaceColor
            border.width: 1
            border.color: page.lineColor
            opacity: page.ready ? 1 : 0
            implicitHeight: actionsCardContent.implicitHeight + Kirigami.Units.largeSpacing * 2

            Behavior on opacity {
                NumberAnimation { duration: 240 }
            }

            ColumnLayout {
                id: actionsCardContent
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.mediumSpacing

                Label {
                    text: qsTr("Actions")
                    color: page.mutedTextColor
                    font.bold: true
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    Button {
                        text: page.primaryActionText()
                        icon.name: page.primaryActionIcon()
                        highlighted: true
                        enabled: shellBackend.daemonReachable && shellBackend.mountPathValid
                        onClicked: page.runPrimaryAction()
                    }

                    Button {
                        text: qsTr("Start filesystem")
                        icon.name: "folder-cloud"
                        visible: shellBackend.canMount
                        enabled: shellBackend.mountPathValid
                        onClicked: shellBackend.mountRemote()
                    }

                    Button {
                        text: qsTr("Stop filesystem")
                        icon.name: "media-eject"
                        visible: shellBackend.canUnmount
                        onClicked: shellBackend.unmountRemote()
                    }

                    Button {
                        text: page.syncActionText()
                        icon.name: page.syncActionIcon()
                        enabled: shellBackend.canPauseSync || shellBackend.canResumeSync
                        onClicked: page.runSyncAction()
                    }

                    Button {
                        text: qsTr("Disconnect")
                        icon.name: "network-disconnect"
                        visible: shellBackend.remoteConfigured
                        onClicked: disconnectDialog.open()
                    }
                }
            }
        }

        Rectangle {
            id: diagnosticsCard
            Layout.fillWidth: true
            radius: Kirigami.Units.largeSpacing
            color: page.surfaceColor
            border.width: 1
            border.color: page.lineColor
            opacity: page.ready ? 1 : 0
            implicitHeight: diagnosticsCardContent.implicitHeight + Kirigami.Units.largeSpacing * 2

            Behavior on opacity {
                NumberAnimation { duration: 260 }
            }

            ColumnLayout {
                id: diagnosticsCardContent
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.mediumSpacing

                Label {
                    text: qsTr("Diagnostics")
                    color: page.mutedTextColor
                    font.bold: true
                }

                Repeater {
                    model: page.diagnosticsRows()

                    delegate: RowLayout {
                        required property var modelData
                        Layout.fillWidth: true

                        Label {
                            text: modelData.label
                            color: page.mutedTextColor
                        }

                        Item {
                            Layout.fillWidth: true
                        }

                        Label {
                            text: modelData.value
                            font.bold: true
                            horizontalAlignment: Text.AlignRight
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    radius: Kirigami.Units.mediumSpacing
                    color: page.mutedSurfaceColor
                    border.width: 1
                    border.color: page.lineColor
                    implicitHeight: explorerHintContent.implicitHeight + Kirigami.Units.mediumSpacing * 2

                    ColumnLayout {
                        id: explorerHintContent
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.mediumSpacing
                        spacing: Kirigami.Units.smallSpacing

                        Label {
                            text: qsTr("Explorer integration")
                            color: page.mutedTextColor
                            font.bold: true
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: page.mutedTextColor
                            text: qsTr("Use Dolphin or Nautilus to keep files on this device, free up space, and inspect overlay states. The tray keeps the daemon reachable after the window closes.")
                        }
                    }
                }
            }
        }

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }
    }
}
