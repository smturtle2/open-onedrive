import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: page
    title: qsTr("Logs")

    property string filterText: ""
    property int filterMode: 0
    property var filteredEntries: []
    readonly property bool hasPinnedIssue: !shellBackend.daemonReachable
                                         || shellBackend.connectionState === "Error"
                                         || shellBackend.mountState === "Error"
                                         || shellBackend.syncState === "Error"
                                         || shellBackend.conflictCount > 0
                                         || shellBackend.lastSyncError.length > 0
    readonly property string pinnedIssueText: shellBackend.lastSyncError.length > 0
                                              ? shellBackend.lastSyncError
                                              : shellBackend.statusMessage

    function formatTimestamp(unixSeconds) {
        if (!unixSeconds || unixSeconds <= 0) {
            return qsTr("Unknown time")
        }
        return Qt.formatDateTime(new Date(unixSeconds * 1000), "yyyy-MM-dd hh:mm:ss")
    }

    function levelLabel(level) {
        const normalized = String(level || "").toLowerCase()
        if (normalized === "warning") {
            return qsTr("Warning")
        }
        if (normalized === "error") {
            return qsTr("Error")
        }
        return qsTr("Info")
    }

    function levelColor(level) {
        const normalized = String(level || "").toLowerCase()
        if (normalized === "warning") {
            return "#8b6f00"
        }
        if (normalized === "error") {
            return "#b3261e"
        }
        return "#295c8a"
    }

    function entryMatchesMode(entry) {
        const level = String(entry.level || "").toLowerCase()
        const source = String(entry.source || "").toLowerCase()
        const message = String(entry.message || "").toLowerCase()
        if (page.filterMode === 1) {
            return level === "warning" || level === "error"
        }
        if (page.filterMode === 2) {
            return source.indexOf("rclone") >= 0
                    || message.indexOf("upload") >= 0
                    || message.indexOf("download") >= 0
                    || message.indexOf("sync") >= 0
                    || message.indexOf("copyto") >= 0
                    || message.indexOf("rescan") >= 0
        }
        return true
    }

    function entryMatchesQuery(entry) {
        const query = page.filterText.trim().toLowerCase()
        if (query.length === 0) {
            return true
        }
        const haystack = [
            String(entry.source || "").toLowerCase(),
            String(entry.level || "").toLowerCase(),
            String(entry.message || "").toLowerCase(),
            page.formatTimestamp(entry.timestamp_unix).toLowerCase()
        ].join(" ")
        return haystack.indexOf(query) >= 0
    }

    function formattedLine(entry) {
        const source = String(entry.source || "").length > 0 ? entry.source : "daemon"
        return qsTr("[%1] [%2] [%3] %4")
                .arg(page.formatTimestamp(entry.timestamp_unix))
                .arg(String(entry.level || "info").toUpperCase())
                .arg(source)
                .arg(String(entry.message || ""))
    }

    function copyEntries(entries) {
        const lines = []
        for (let index = 0; index < entries.length; ++index) {
            lines.push(page.formattedLine(entries[index]))
        }
        shellBackend.copyLinesToClipboard(lines)
    }

    function rebuildFilteredEntries() {
        const next = []
        for (let index = 0; index < shellBackend.recentLogEntries.length; ++index) {
            const entry = shellBackend.recentLogEntries[index]
            if (!page.entryMatchesMode(entry) || !page.entryMatchesQuery(entry)) {
                continue
            }
            next.push(entry)
        }
        page.filteredEntries = next
    }

    Component.onCompleted: rebuildFilteredEntries()

    Connections {
        target: shellBackend

        function onRecentLogsChanged() {
            page.rebuildFilteredEntries()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

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
                text: qsTr("Copy filtered")
                icon.name: "edit-copy"
                enabled: page.filteredEntries.length > 0
                onClicked: page.copyEntries(page.filteredEntries)
            }

            Button {
                text: qsTr("Copy all")
                icon.name: "edit-copy"
                enabled: shellBackend.recentLogEntries.length > 0
                onClicked: page.copyEntries(shellBackend.recentLogEntries)
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

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("Search recent entries by source, severity, message text, or timestamp. Copy only the filtered slice when you need to attach focused diagnostics.")
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    TextField {
                        Layout.fillWidth: true
                        placeholderText: qsTr("Search logs")
                        text: page.filterText
                        onTextChanged: {
                            page.filterText = text
                            page.rebuildFilteredEntries()
                        }
                    }

                    ComboBox {
                        model: [
                            qsTr("All entries"),
                            qsTr("Warnings and errors"),
                            qsTr("Transfers")
                        ]

                        onCurrentIndexChanged: {
                            page.filterMode = currentIndex
                            page.rebuildFilteredEntries()
                        }
                    }
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("%1 of %2 recent entries shown.").arg(page.filteredEntries.length).arg(shellBackend.recentLogEntries.length)
                }
            }
        }

        Label {
            Layout.fillWidth: true
            visible: shellBackend.recentLogEntries.length === 0
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.neutralTextColor
            text: qsTr("No structured log entries yet. Start the sign-in flow or the filesystem to capture daemon and rclone output.")
        }

        Label {
            Layout.fillWidth: true
            visible: shellBackend.recentLogEntries.length > 0 && page.filteredEntries.length === 0
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.neutralTextColor
            text: qsTr("No log entries match the current filter.")
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: page.filteredEntries.length > 0
            clip: true
            spacing: Kirigami.Units.smallSpacing
            model: page.filteredEntries

            delegate: Frame {
                required property var modelData
                readonly property var entry: modelData

                width: ListView.view.width
                padding: Kirigami.Units.mediumSpacing

                ColumnLayout {
                    anchors.fill: parent
                    spacing: Kirigami.Units.smallSpacing

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing

                        Rectangle {
                            radius: 999
                            color: page.levelColor(entry.level)
                            implicitHeight: levelLabel.implicitHeight + Kirigami.Units.smallSpacing * 2
                            implicitWidth: levelLabel.implicitWidth + Kirigami.Units.mediumSpacing * 2

                            Label {
                                id: levelLabel
                                anchors.centerIn: parent
                                text: page.levelLabel(entry.level)
                                color: "white"
                                font.bold: true
                            }
                        }

                        Label {
                            text: String(entry.source || "").length > 0 ? entry.source : qsTr("daemon")
                            font.bold: true
                        }

                        Item {
                            Layout.fillWidth: true
                        }

                        Label {
                            text: page.formatTimestamp(entry.timestamp_unix)
                            color: Kirigami.Theme.neutralTextColor
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: String(entry.message || "")
                    }
                }
            }
        }
    }
}
