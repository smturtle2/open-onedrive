import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Frame {
    id: root

    property string helperText: ""

    Layout.fillWidth: true
    padding: Kirigami.Units.largeSpacing

    FolderDialog {
        id: folderDialog
        title: "Choose mount directory"
        acceptLabel: "Use Folder"

        onAccepted: shellBackend.setMountPathFromUrl(selectedFolder)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.mediumSpacing

        Label {
            text: "Mount directory"
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            TextField {
                Layout.fillWidth: true
                placeholderText: "/home/you/OneDrive"
                text: shellBackend.mountPath
                onTextEdited: shellBackend.mountPath = text
            }

            Button {
                text: "Browse..."
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
            wrapMode: Text.WordWrap
            color: shellBackend.mountPathPending
                   ? Kirigami.Theme.neutralTextColor
                   : Kirigami.Theme.disabledTextColor
            text: shellBackend.mountPathPending
                  ? "Pending change. Apply it with Connect, Mount, or Retry Mount."
                  : "Changes take effect the next time you connect or remount."
        }

        Label {
            Layout.fillWidth: true
            visible: helperText.length > 0
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.neutralTextColor
            text: helperText
        }
    }
}
