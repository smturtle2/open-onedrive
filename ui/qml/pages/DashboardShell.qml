import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: page
    title: qsTr("open-onedrive")

    property int currentIndex: 0

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        TabBar {
            Layout.fillWidth: true
            currentIndex: page.currentIndex
            onCurrentIndexChanged: page.currentIndex = currentIndex

            TabButton {
                text: qsTr("Dashboard")
            }

            TabButton {
                text: qsTr("Logs")
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: page.currentIndex

            DashboardPage { }
            LogsPage { }
        }
    }
}
