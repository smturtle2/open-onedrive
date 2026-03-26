import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Frame {
    id: root

    property string helperText: ""
    readonly property string trimmedMountPath: shellBackend.mountPath.trim()
    readonly property bool hasDraftPath: trimmedMountPath.length > 0
    readonly property bool hasAbsoluteDraftPath: hasDraftPath && trimmedMountPath.startsWith("/")
    readonly property string feedbackLabel: !hasDraftPath
                                           ? qsTr("Choose a folder")
                                           : !hasAbsoluteDraftPath
                                             ? qsTr("Absolute path required")
                                             : shellBackend.mountPathPending
                                               ? qsTr("Pending apply")
                                               : qsTr("Ready")
    readonly property color feedbackColor: !hasDraftPath
                                           ? "#8b6f00"
                                           : !hasAbsoluteDraftPath
                                             ? "#b3261e"
                                             : shellBackend.mountPathPending
                                               ? "#295c8a"
                                               : "#1f7a4d"

    Layout.fillWidth: true
    padding: Kirigami.Units.largeSpacing

    FolderDialog {
        id: folderDialog
        title: qsTr("Choose OneDrive root folder")
        acceptLabel: qsTr("Use Folder")

        onAccepted: shellBackend.setMountPathFromUrl(selectedFolder)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.mediumSpacing

        Label {
            text: qsTr("Root folder")
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Rectangle {
                radius: 999
                color: Qt.rgba(root.feedbackColor.r, root.feedbackColor.g, root.feedbackColor.b, 0.14)
                border.width: 1
                border.color: Qt.rgba(root.feedbackColor.r, root.feedbackColor.g, root.feedbackColor.b, 0.34)
                implicitHeight: feedbackText.implicitHeight + Kirigami.Units.smallSpacing * 2
                implicitWidth: feedbackText.implicitWidth + Kirigami.Units.largeSpacing

                Label {
                    id: feedbackText
                    anchors.centerIn: parent
                    text: root.feedbackLabel
                    color: root.feedbackColor
                    font.bold: true
                }
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.neutralTextColor
                text: shellBackend.effectiveMountPath.length > 0
                      ? qsTr("Current root: %1").arg(shellBackend.effectiveMountPath)
                      : qsTr("No visible root has been applied yet.")
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            TextField {
                Layout.fillWidth: true
                placeholderText: qsTr("/home/you/OneDrive")
                text: shellBackend.mountPath
                onTextEdited: shellBackend.mountPath = text
            }

            Button {
                text: qsTr("Browse...")
                icon.name: "document-open-folder"
                onClicked: {
                    const folder = shellBackend.mountPathDialogFolder()
                    folderDialog.currentFolder = folder
                    folderDialog.selectedFolder = folder
                    folderDialog.open()
                }
            }
        }

        Label {
            Layout.fillWidth: true
            visible: root.hasDraftPath && !root.hasAbsoluteDraftPath
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.negativeTextColor
            text: qsTr("Use a full absolute path such as /home/you/OneDrive.")
        }

        Label {
            Layout.fillWidth: true
            visible: root.hasAbsoluteDraftPath
            wrapMode: Text.WordWrap
            color: shellBackend.mountPathPending
                   ? Kirigami.Theme.neutralTextColor
                   : Kirigami.Theme.disabledTextColor
            text: shellBackend.mountPathPending
                  ? qsTr("Pending change. Apply it with Connect, Start Filesystem, Repair Remote, or Retry Filesystem.")
                  : qsTr("Changes take effect the next time you connect or restart the filesystem.")
        }

        Label {
            Layout.fillWidth: true
            visible: helperText.length > 0
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.neutralTextColor
            text: helperText + qsTr(" Choose an empty directory. The daemon manages a hidden %1 folder inside this root for hydrated file bytes.").arg(shellBackend.backingDirName)
        }
    }
}
