import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "../components"

Kirigami.ScrollablePage {
    id: page
    title: qsTr("Dashboard")
    property string quickPath: ""

    function stateColor() {
        if (shellBackend.mountState === "Running") {
            return "#1f7a4d"
        }
        if (shellBackend.mountState === "Starting" || shellBackend.connectionState === "Connecting") {
            return "#c77700"
        }
        if (shellBackend.mountState === "Error" || shellBackend.connectionState === "Error" || shellBackend.conflictCount > 0) {
            return "#b3261e"
        }
        return "#3a5a78"
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

            Frame {
                Layout.fillWidth: true
                padding: Kirigami.Units.largeSpacing

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    RowLayout {
                        Layout.fillWidth: true

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            Kirigami.Heading {
                                text: qsTr("OneDrive filesystem control")
                                level: 1
                            }

                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: shellBackend.statusMessage
                                color: Kirigami.Theme.neutralTextColor
                            }
                        }

                        Rectangle {
                            radius: 999
                            color: page.stateColor()
                            implicitHeight: badgeLabel.implicitHeight + Kirigami.Units.smallSpacing * 2
                            implicitWidth: badgeLabel.implicitWidth + Kirigami.Units.largeSpacing * 2

                            Label {
                                id: badgeLabel
                                anchors.centerIn: parent
                                text: shellBackend.mountStateLabel
                                color: "white"
                                font.bold: true
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true

                        Button {
                            text: qsTr("Refresh")
                            icon.name: "view-refresh"
                            onClicked: shellBackend.refreshStatus()
                        }

                        Button {
                            text: qsTr("Rescan")
                            icon.name: "folder-sync"
                            enabled: shellBackend.remoteConfigured
                            onClicked: shellBackend.rescanRemote()
                        }

                        Button {
                            text: qsTr("Start Filesystem")
                            icon.name: "folder-cloud"
                            enabled: shellBackend.canMount
                            onClicked: shellBackend.mountRemote()
                        }

                        Button {
                            text: qsTr("Stop Filesystem")
                            icon.name: "media-eject"
                            enabled: shellBackend.canUnmount
                            onClicked: shellBackend.unmountRemote()
                        }

                        Button {
                            text: qsTr("Retry")
                            icon.name: "view-refresh"
                            enabled: shellBackend.canRetry
                            onClicked: shellBackend.retryMount()
                        }

                        Button {
                            text: qsTr("Pause Sync")
                            icon.name: "media-playback-pause"
                            enabled: shellBackend.canPauseSync
                            onClicked: shellBackend.pauseSync()
                        }

                        Button {
                            text: qsTr("Resume Sync")
                            icon.name: "media-playback-start"
                            enabled: shellBackend.canResumeSync
                            onClicked: shellBackend.resumeSync()
                        }

                        Button {
                            text: qsTr("Open Folder")
                            icon.name: "document-open-folder"
                            enabled: shellBackend.effectiveMountPath.length > 0
                            onClicked: shellBackend.openMountLocation()
                        }

                        Button {
                            text: qsTr("Disconnect")
                            icon.name: "network-disconnect"
                            onClicked: shellBackend.disconnectRemote()
                        }
                    }
                }
            }
        }

        GridLayout {
            Layout.fillWidth: true
            columns: width > 960 ? 6 : width > 680 ? 2 : 1
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.largeSpacing

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Backend")
                value: "rclone"
                description: shellBackend.rcloneVersion.length > 0 ? shellBackend.rcloneVersion : qsTr("Version pending")
                accentColor: "#4f7cff"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Connection")
                value: shellBackend.connectionStateLabel
                description: shellBackend.remoteConfigured ? qsTr("Remote profile is ready for the local filesystem.") : qsTr("Browser sign-in has not completed yet.")
                accentColor: shellBackend.connectionState === "Error"
                             ? "#b3261e"
                             : shellBackend.connectionState === "Connecting"
                               ? "#c77700"
                               : "#3a5a78"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Filesystem")
                value: shellBackend.mountStateLabel
                description: shellBackend.statusMessage
                accentColor: page.stateColor()
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Sync")
                value: shellBackend.syncStateLabel
                description: qsTr("%1 downloads pending · %2 uploads pending").arg(shellBackend.pendingDownloads).arg(shellBackend.pendingUploads)
                accentColor: shellBackend.syncState === "Error"
                             ? "#b3261e"
                             : shellBackend.syncState === "Paused"
                               ? "#8b6f00"
                               : shellBackend.syncState === "Syncing" || shellBackend.syncState === "Scanning"
                                 ? "#4f7cff"
                                 : "#3a5a78"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Root folder")
                value: shellBackend.effectiveMountPath
                description: qsTr("Visible local path exposed by the custom filesystem")
                accentColor: "#3a5a78"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Backing store")
                value: shellBackend.cacheUsageLabel
                description: qsTr("Hidden %1 directory that keeps hydrated file bytes").arg(shellBackend.backingDirName)
                accentColor: "#6f8b42"
            }

            StatusCard {
                Layout.fillWidth: true
                title: qsTr("Residency")
                value: qsTr("%1 pinned").arg(shellBackend.pinnedFileCount)
                description: qsTr("%1 pending total · %2 conflicts").arg(shellBackend.queueDepth).arg(shellBackend.conflictCount)
                accentColor: "#9b6bff"
            }
        }

        MountPathEditor {
            helperText: qsTr("Path changes stay local until you trigger Connect, Start Filesystem, or Retry Filesystem from this dashboard.")
        }

        Frame {
            Layout.fillWidth: true
            padding: Kirigami.Units.largeSpacing

            ColumnLayout {
                anchors.fill: parent
                spacing: Kirigami.Units.mediumSpacing

                Kirigami.Heading {
                    text: qsTr("Quick File Controls")
                    level: 3
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("Enter an absolute path inside the OneDrive root folder or a path relative to that root.")
                }

                RowLayout {
                    Layout.fillWidth: true

                    TextField {
                        Layout.fillWidth: true
                        placeholderText: qsTr("Documents/report.pdf")
                        text: page.quickPath
                        onTextEdited: page.quickPath = text
                    }

                    Button {
                        text: qsTr("Keep on device")
                        icon.name: "emblem-favorite"
                        enabled: page.quickPath.trim().length > 0
                        onClicked: shellBackend.keepLocalPath(page.quickPath)
                    }

                    Button {
                        text: qsTr("Make online-only")
                        icon.name: "folder-download"
                        enabled: page.quickPath.trim().length > 0
                        onClicked: shellBackend.makeOnlineOnlyPath(page.quickPath)
                    }

                    Button {
                        text: qsTr("Retry transfer")
                        icon.name: "view-refresh"
                        enabled: page.quickPath.trim().length > 0
                        onClicked: shellBackend.retryTransferPath(page.quickPath)
                    }
                }
            }
        }

        Frame {
            Layout.fillWidth: true

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
                    text: qsTr("Last sync: %1").arg(shellBackend.lastSyncLabel)
                }

                Label {
                    Layout.fillWidth: true
                    visible: shellBackend.lastSyncError.length > 0
                    wrapMode: Text.WordWrap
                    color: "#b3261e"
                    text: shellBackend.lastSyncError
                }

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.neutralTextColor
                    text: qsTr("Dolphin overlays update from the daemon path-state cache. Use Dolphin or the quick controls above to keep files local or return them to online-only mode.")
                }
            }
        }
    }
}
