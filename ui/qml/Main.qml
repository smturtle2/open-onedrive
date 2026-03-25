import QtQuick
import QtQuick.Controls
import org.kde.kirigami as Kirigami

Kirigami.ApplicationWindow {
    id: root
    width: 1040
    height: 720
    minimumWidth: 820
    minimumHeight: 620
    title: qsTr("open-onedrive")

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    Component.onCompleted: shellBackend.refreshStatus()

    Loader {
        anchors.fill: parent
        source: "qrc:/qml/pages/DashboardShell.qml"
    }
}
