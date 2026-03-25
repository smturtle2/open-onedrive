#include "ActionPlugin.hpp"

#include <KFileItemListProperties>
#include <KPluginFactory>
#include <QAction>
#include <QDBusInterface>
#include <QDBusReply>
#include <QDesktopServices>
#include <QJsonDocument>
#include <QJsonObject>
#include <QWidget>

namespace {
constexpr auto kService = "io.github.smturtle2.OpenOneDrive1";
constexpr auto kPath = "/io/github/smturtle2/OpenOneDrive1";
constexpr auto kInterface = "io.github.smturtle2.OpenOneDrive1";

QString mountRoot()
{
    QDBusInterface iface(QString::fromLatin1(kService),
                         QString::fromLatin1(kPath),
                         QString::fromLatin1(kInterface),
                         QDBusConnection::sessionBus());
    const QDBusReply<QString> reply = iface.call(QStringLiteral("GetStatusJson"));
    if (!reply.isValid()) {
        return {};
    }

    const QJsonDocument document = QJsonDocument::fromJson(reply.value().toUtf8());
    if (!document.isObject()) {
        return {};
    }

    return document.object().value(QStringLiteral("mount_path")).toString();
}
}

K_PLUGIN_CLASS_WITH_JSON(OpenOneDriveActionPlugin, "../metadata.json")

QList<QAction *> OpenOneDriveActionPlugin::actions(const KFileItemListProperties &fileItemInfos, QWidget *parentWidget)
{
    QObject *actionParent = parentWidget != nullptr ? static_cast<QObject *>(parentWidget) : this;
    auto *openMountAction = new QAction(QStringLiteral("Open OneDrive Mount"), actionParent);
    QObject::connect(openMountAction, &QAction::triggered, this, [] {
        const QString mountPath = mountRoot();
        if (!mountPath.isEmpty()) {
            QDesktopServices::openUrl(QUrl::fromLocalFile(mountPath));
        }
    });

    auto *reconnectAction = new QAction(QStringLiteral("Reconnect OneDrive"), actionParent);
    QObject::connect(reconnectAction, &QAction::triggered, this, [] {
        QDBusInterface iface(QString::fromLatin1(kService),
                             QString::fromLatin1(kPath),
                             QString::fromLatin1(kInterface),
                             QDBusConnection::sessionBus());
        iface.call(QStringLiteral("RetryMount"));
    });

    auto *unmountAction = new QAction(QStringLiteral("Unmount OneDrive"), actionParent);
    QObject::connect(unmountAction, &QAction::triggered, this, [] {
        QDBusInterface iface(QString::fromLatin1(kService),
                             QString::fromLatin1(kPath),
                             QString::fromLatin1(kInterface),
                             QDBusConnection::sessionBus());
        iface.call(QStringLiteral("Unmount"));
    });

    QList<QAction *> actions;
    actions << openMountAction << reconnectAction << unmountAction;
    return actions;
}

#include "ActionPlugin.moc"
