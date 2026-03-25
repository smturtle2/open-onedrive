import QtQuick
import QtQuick.Controls
import org.kde.kirigami as Kirigami

Kirigami.ApplicationWindow {
    id: root
    width: 1040
    height: 720
    minimumWidth: 900
    minimumHeight: 640
    title: "open-onedrive"

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    Component.onCompleted: shellBackend.refreshStatus()

    Loader {
        anchors.fill: parent
        source: shellBackend.dashboardReady
                  ? "qrc:/qml/pages/DashboardShell.qml"
                  : "qrc:/qml/pages/SetupPage.qml"
    }
}
