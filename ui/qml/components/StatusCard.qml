import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Frame {
    required property string title
    required property string value
    required property string description

    padding: Kirigami.Units.largeSpacing

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.smallSpacing

        Label {
            text: title
            color: Kirigami.Theme.neutralTextColor
        }

        Kirigami.Heading {
            text: value.length > 0 ? value : "Not configured"
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

