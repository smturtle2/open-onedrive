import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    id: page
    title: qsTr("Logs")

    property string filterText: ""
    property int filterMode: 0
    property var filteredLogs: []
    readonly property bool hasPinnedIssue: !shellBackend.daemonReachable
                                         || shellBackend.connectionState === "Error"
                                         || shellBackend.mountState === "Error"
                                         || shellBackend.syncState === "Error"
                                         || shellBackend.conflictCount > 0
                                         || shellBackend.lastSyncError.length > 0
    readonly property string pinnedIssueText: shellBackend.lastSyncError.length > 0
                                              ? shellBackend.lastSyncError
                                              : shellBackend.statusMessage

    function lineMatchesMode(line) {
        const lower = line.toLowerCase()
        if (page.filterMode === 1) {
            return lower.indexOf("error") >= 0
                    || lower.indexOf("failed") >= 0
                    || lower.indexOf("conflict") >= 0
                    || lower.indexOf("warning") >= 0
        }
        if (page.filterMode === 2) {
            return lower.indexOf("upload") >= 0
                    || lower.indexOf("download") >= 0
                    || lower.indexOf("sync") >= 0
                    || lower.indexOf("copyto") >= 0
                    || lower.indexOf("rescan") >= 0
        }
        return true
    }

    function rebuildFilteredLogs() {
        const query = page.filterText.trim().toLowerCase()
        const next = []
        for (let index = 0; index < shellBackend.recentLogs.length; ++index) {
            const line = shellBackend.recentLogs[index]
            if (!page.lineMatchesMode(line)) {
                continue
            }
            if (query.length > 0 && line.toLowerCase().indexOf(query) < 0) {
                continue
            }
            next.push(line)
        }
        page.filteredLogs = next
    }

    Component.onCompleted: rebuildFilteredLogs()

    Connections {
        target: shellBackend

        function onRecentLogsChanged() {
            page.rebuildFilteredLogs()
        }
    }

    ColumnLayout {
        width: Math.min(parent.width, 960)
        x: Math.max(0, (parent.width - width) / 2)
        spacing: Kirigami.Units.largeSpacing

        Item {
            Layout.preferredHeight: Kirigami.Units.largeSpacing
        }

        RowLayout {
            Layout.fillWidth: true

            Kirigami.Heading {
                text: qsTr("Recent daemon and rclone logs")
                level: 1
            }

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: qsTr("Copy")
                icon.name: "edit-copy"
                enabled: shellBackend.recentLogs.length > 0
                onClicked: shellBackend.copyRecentLogsToClipboard()
            }

            Button {
                text: qsTr("Refresh")
                icon.name: "view-refresh"
                onClicked: shellBackend.refreshLogs()
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.hasPinnedIssue
            type: shellBackend.lastSyncError.length > 0
                  || shellBackend.connectionState === "Error"
                  || shellBackend.mountState === "Error"
                  || shellBackend.syncState === "Error"
                  || shellBackend.conflictCount > 0
                  || !shellBackend.daemonReachable
                  ? Kirigami.MessageType.Error
                  : Kirigami.MessageType.Information
            showCloseButton: false
            text: page.pinnedIssueText
        }

        Frame {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("Use search and filters to isolate the lines that matter while you retry transfers or restart the filesystem.")
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    TextField {
                        Layout.fillWidth: true
                        placeholderText: qsTr("Search logs")
                        text: page.filterText
                        onTextEdited: {
                            page.filterText = text
                            page.rebuildFilteredLogs()
                        }
                    }

                    ComboBox {
                        model: [
                            qsTr("All lines"),
                            qsTr("Errors only"),
                            qsTr("Transfers")
                        ]
                        onActivated: {
                            page.filterMode = currentIndex
                            page.rebuildFilteredLogs()
                        }
                    }
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("%1 of %2 recent line(s) shown.").arg(page.filteredLogs.length).arg(shellBackend.recentLogs.length)
                }

                Label {
                    Layout.fillWidth: true
                    visible: shellBackend.recentLogs.length === 0
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("No log lines yet. Start the sign-in flow or the filesystem to capture daemon and rclone output.")
                }

                Label {
                    Layout.fillWidth: true
                    visible: shellBackend.recentLogs.length > 0 && page.filteredLogs.length === 0
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("No log lines match the current filter.")
                }

                ListView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: page.filteredLogs.length > 0
                    clip: true
                    spacing: Kirigami.Units.smallSpacing
                    model: page.filteredLogs

                    delegate: Frame {
                        required property string modelData
                        width: ListView.view.width
                        padding: Kirigami.Units.mediumSpacing

                        Label {
                            width: parent.width
                            wrapMode: Text.WrapAnywhere
                            text: modelData
                            font.family: "monospace"
                        }
                    }
                }
            }
        }
    }
}
