import QtQuick
import QtQuick.Controls
import org.kde.kirigami as Kirigami

Kirigami.ApplicationWindow {
    id: root
    width: 980
    height: 760
    minimumWidth: 760
    minimumHeight: 620
    title: qsTr("open-onedrive")

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    Component.onCompleted: {
        shellBackend.refreshStatus()
        shellBackend.refreshLogs()
    }

    background: Rectangle {
        color: "#edf2f5"
    }

    Loader {
        anchors.fill: parent
        source: "qrc:/qml/pages/SetupPage.qml"
    }
}
