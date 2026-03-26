import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property string helperText: ""
    readonly property string trimmedMountPath: shellBackend.mountPath.trim()
    readonly property bool hasDraftPath: trimmedMountPath.length > 0
    readonly property bool hasPathIssue: shellBackend.mountPathIssue.length > 0
    readonly property bool hasAppliedPath: hasDraftPath
                                           && !hasPathIssue
                                           && !shellBackend.mountPathPending
                                           && shellBackend.effectiveMountPath.length > 0
    readonly property string stateLabel: !hasDraftPath
                                         ? qsTr("Not set")
                                         : hasPathIssue
                                           ? qsTr("Path issue")
                                           : shellBackend.mountPathPending
                                             ? qsTr("Pending apply")
                                             : qsTr("Current root")
    readonly property color stateColor: !hasDraftPath
                                         ? "#617182"
                                         : hasPathIssue
                                           ? "#b53b2d"
                                           : shellBackend.mountPathPending
                                             ? "#245f92"
                                             : "#51606f"
    readonly property string helperLabelText: {
        if (hasPathIssue) {
            return shellBackend.mountPathIssue
        }
        if (shellBackend.mountPathPending) {
            return qsTr("The next connect or filesystem restart applies this path.")
        }
        if (hasAppliedPath) {
            return qsTr("This path is currently active. Hydrated bytes stay in the hidden %1 folder inside this root.")
                .arg(shellBackend.backingDirName)
        }
        if (helperText.length > 0) {
            return helperText + qsTr(" Hydrated bytes stay in the hidden %1 folder inside this root.")
                .arg(shellBackend.backingDirName)
        }
        return ""
    }

    Layout.fillWidth: true
    implicitHeight: content.implicitHeight

    FolderDialog {
        id: folderDialog
        title: qsTr("Choose OneDrive root folder")
        acceptLabel: qsTr("Use Folder")

        onAccepted: shellBackend.setMountPathFromUrl(selectedFolder)
    }

    ColumnLayout {
        id: content
        anchors.fill: parent
        spacing: Kirigami.Units.mediumSpacing

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            Rectangle {
                radius: 999
                color: Qt.rgba(root.stateColor.r, root.stateColor.g, root.stateColor.b, 0.12)
                border.width: 1
                border.color: Qt.rgba(root.stateColor.r, root.stateColor.g, root.stateColor.b, 0.24)
                implicitWidth: feedback.implicitWidth + Kirigami.Units.largeSpacing
                implicitHeight: feedback.implicitHeight + Kirigami.Units.smallSpacing * 2

                Label {
                    id: feedback
                    anchors.centerIn: parent
                    text: root.stateLabel
                    color: root.stateColor
                    font.bold: true
                }
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                color: "#617182"
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
                text: qsTr("Browse")
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
            visible: root.hasPathIssue
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.negativeTextColor
            text: root.helperLabelText
        }

        Label {
            Layout.fillWidth: true
            visible: !root.hasPathIssue && root.helperLabelText.length > 0
            wrapMode: Text.WordWrap
            color: "#617182"
            text: root.helperLabelText
        }
    }
}
