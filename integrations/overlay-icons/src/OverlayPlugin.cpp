#include "OverlayPlugin.hpp"

#include <KPluginFactory>
#include <QDBusInterface>
#include <QDBusReply>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

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

QString virtualPathForItem(const QUrl &item)
{
    if (!item.isLocalFile()) {
        return {};
    }

    const QString root = mountRoot();
    if (root.isEmpty()) {
        return {};
    }

    const QString localPath = item.toLocalFile();
    if (!localPath.startsWith(root)) {
        return {};
    }

    QString virtualPath = localPath.mid(root.length());
    if (virtualPath.isEmpty()) {
        virtualPath = QStringLiteral("/");
    }
    if (!virtualPath.startsWith('/')) {
        virtualPath.prepend('/');
    }
    return virtualPath;
}
}

K_PLUGIN_CLASS_WITH_JSON(OpenOneDriveOverlayPlugin, "../metadata.json")

QStringList OpenOneDriveOverlayPlugin::getOverlays(const QUrl &item)
{
    const QString virtualPath = virtualPathForItem(item);
    if (virtualPath.isEmpty()) {
        return {};
    }

    QDBusInterface iface(QString::fromLatin1(kService),
                         QString::fromLatin1(kPath),
                         QString::fromLatin1(kInterface),
                         QDBusConnection::sessionBus());
    const QDBusReply<QString> reply = iface.call(QStringLiteral("GetItemsJson"), QStringList {virtualPath});
    if (!reply.isValid()) {
        return {};
    }

    const QJsonDocument document = QJsonDocument::fromJson(reply.value().toUtf8());
    if (!document.isArray() || document.array().isEmpty()) {
        return {};
    }

    const QJsonObject object = document.array().at(0).toObject();
    const QString availability = object.value(QStringLiteral("availability")).toString();
    if (availability == QStringLiteral("Pinned")) {
        return {QStringLiteral("emblem-favorite")};
    }
    if (availability == QStringLiteral("OnlineOnly")) {
        return {QStringLiteral("emblem-downloads")};
    }
    if (availability == QStringLiteral("Hydrating")) {
        return {QStringLiteral("view-refresh")};
    }
    if (availability == QStringLiteral("Error")) {
        return {QStringLiteral("emblem-important")};
    }

    return {};
}

#include "OverlayPlugin.moc"
