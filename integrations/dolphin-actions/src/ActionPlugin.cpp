#include "ActionPlugin.hpp"

#include <KFileItem>
#include <KFileItemListProperties>
#include <KPluginFactory>
#include <QAction>
#include <QDBusInterface>
#include <QDBusReply>
#include <QDesktopServices>
#include <QDir>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStringList>
#include <QWidget>

namespace {
constexpr auto kService = "io.github.smturtle2.OpenOneDrive1";
constexpr auto kPath = "/io/github/smturtle2/OpenOneDrive1";
constexpr auto kInterface = "io.github.smturtle2.OpenOneDrive1";

QDBusInterface daemonInterface()
{
    return QDBusInterface(QString::fromLatin1(kService),
                          QString::fromLatin1(kPath),
                          QString::fromLatin1(kInterface),
                          QDBusConnection::sessionBus());
}

QJsonObject statusObject()
{
    QDBusInterface iface = daemonInterface();
    const QDBusReply<QString> reply = iface.call(QStringLiteral("GetStatusJson"));
    if (!reply.isValid()) {
        return {};
    }

    const QJsonDocument document = QJsonDocument::fromJson(reply.value().toUtf8());
    if (!document.isObject()) {
        return {};
    }

    return document.object();
}

QString mountRoot(const QJsonObject &status)
{
    return QDir::cleanPath(status.value(QStringLiteral("mount_path")).toString());
}

QStringList selectedLocalPaths(const KFileItemListProperties &fileItemInfos)
{
    QStringList paths;
    for (const auto &item : fileItemInfos.items()) {
        const QString localPath = item.localPath();
        if (!localPath.isEmpty()) {
            paths << QDir::cleanPath(localPath);
        }
    }
    return paths;
}

bool isWithinMountRoot(const QString &path, const QString &mountPath)
{
    if (path.isEmpty() || mountPath.isEmpty()) {
        return false;
    }

    return path == mountPath || path.startsWith(mountPath + QLatin1Char('/'));
}

bool invokePathAction(const QString &method, const QStringList &paths)
{
    if (paths.isEmpty()) {
        return false;
    }

    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        return false;
    }

    const QDBusReply<uint> reply = iface.call(method, paths);
    return reply.isValid();
}
}

K_PLUGIN_CLASS_WITH_JSON(OpenOneDriveActionPlugin, "../metadata.json")

QList<QAction *> OpenOneDriveActionPlugin::actions(const KFileItemListProperties &fileItemInfos, QWidget *parentWidget)
{
    const QJsonObject status = statusObject();
    if (!status.value(QStringLiteral("remote_configured")).toBool()) {
        return {};
    }

    const QString mountPath = mountRoot(status);
    const QStringList selectedPaths = selectedLocalPaths(fileItemInfos);
    if (mountPath.isEmpty() || selectedPaths.isEmpty()) {
        return {};
    }

    for (const QString &path : selectedPaths) {
        if (!isWithinMountRoot(path, mountPath)) {
            return {};
        }
    }

    QObject *actionParent = parentWidget != nullptr ? static_cast<QObject *>(parentWidget) : this;
    auto *keepLocalAction = new QAction(QStringLiteral("Keep on this device"), actionParent);
    QObject::connect(keepLocalAction, &QAction::triggered, this, [selectedPaths] {
        invokePathAction(QStringLiteral("KeepLocal"), selectedPaths);
    });

    auto *onlineOnlyAction = new QAction(QStringLiteral("Make online-only"), actionParent);
    QObject::connect(onlineOnlyAction, &QAction::triggered, this, [selectedPaths] {
        invokePathAction(QStringLiteral("MakeOnlineOnly"), selectedPaths);
    });

    auto *openMountAction = new QAction(QStringLiteral("Open OneDrive Folder"), actionParent);
    QObject::connect(openMountAction, &QAction::triggered, this, [mountPath] {
        QDesktopServices::openUrl(QUrl::fromLocalFile(mountPath));
    });

    QList<QAction *> actions;
    actions << keepLocalAction << onlineOnlyAction << openMountAction;
    return actions;
}

#include "ActionPlugin.moc"
