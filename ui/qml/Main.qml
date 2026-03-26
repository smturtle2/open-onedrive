import QtQuick
import QtQuick.Controls
import org.kde.kirigami as Kirigami

Kirigami.ApplicationWindow {
    id: root
    width: 1220
    height: 780
    minimumWidth: 920
    minimumHeight: 660
    title: qsTr("open-onedrive")

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    Component.onCompleted: shellBackend.refreshStatus()

    Loader {
        anchors.fill: parent
        source: "qrc:/qml/pages/DashboardShell.qml"
    }
}
