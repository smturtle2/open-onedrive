import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Frame {
    required property string title
    required property string value
    required property string description
    property color accentColor: Kirigami.Theme.highlightColor

    background: Rectangle {
        radius: Kirigami.Units.largeSpacing
        color: "#ffffff"
        border.width: 1
        border.color: Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.18)
    }
    padding: Kirigami.Units.largeSpacing

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.smallSpacing

        Rectangle {
            Layout.preferredWidth: 44
            Layout.preferredHeight: 4
            radius: 999
            color: accentColor
        }

        Label {
            text: title
            color: Kirigami.Theme.neutralTextColor
            font.bold: true
        }

        Kirigami.Heading {
            text: value.length > 0 ? value : qsTr("Not configured")
            level: 2
            wrapMode: Text.Wrap
        }

        Label {
            text: description
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
        }
    }
}
