#include "OverlayIconPlugin.hpp"

#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDBusReply>
#include <QDateTime>
#include <QDir>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

namespace {
constexpr auto kService = "io.github.smturtle2.OpenOneDrive1";
constexpr auto kPath = "/io/github/smturtle2/OpenOneDrive1";
constexpr auto kInterface = "io.github.smturtle2.OpenOneDrive1";
constexpr qint64 kStatusCacheTtlMs = 2000;
}

QStringList OpenOneDriveOverlayIconPlugin::getOverlays(const QUrl &item)
{
    if (!item.isLocalFile()) {
        return {};
    }

    const QString absolutePath = QDir::cleanPath(item.toLocalFile());
    if (absolutePath.isEmpty()) {
        return {};
    }

    const QString mountRoot = currentMountRoot();
    if (mountRoot.isEmpty()) {
        return {};
    }
    const QString backingDirName = currentBackingDirName();
    if (!backingDirName.isEmpty()) {
        const QString hiddenRoot = QDir::cleanPath(mountRoot + QLatin1Char('/') + backingDirName);
        if (absolutePath == hiddenRoot || absolutePath.startsWith(hiddenRoot + QLatin1Char('/'))) {
            return {};
        }
    }

    if (m_cache.contains(absolutePath)) {
        return m_cache.value(absolutePath);
    }

    if (!m_pending.contains(absolutePath)) {
        requestPathState(absolutePath);
    }
    return {};
}

void OpenOneDriveOverlayIconPlugin::onPathStatesChanged(const QStringList &paths)
{
    const QString mountRoot = currentMountRoot();
    if (paths.isEmpty() || mountRoot.isEmpty()) {
        const auto cachedPaths = m_cache.keys();
        for (const QString &cachedPath : cachedPaths) {
            emit overlaysChanged(QUrl::fromLocalFile(cachedPath), {});
        }
        m_cache.clear();
        return;
    }

    for (const QString &relativePath : paths) {
        const QString absolutePath = QDir::cleanPath(mountRoot + QLatin1Char('/') + relativePath);
        if (m_cache.remove(absolutePath) > 0) {
            emit overlaysChanged(QUrl::fromLocalFile(absolutePath), {});
        }
    }
}

void OpenOneDriveOverlayIconPlugin::requestPathState(const QString &absolutePath)
{
    QDBusInterface iface(QString::fromLatin1(kService),
                         QString::fromLatin1(kPath),
                         QString::fromLatin1(kInterface),
                         QDBusConnection::sessionBus());
    if (!iface.isValid()) {
        return;
    }

    if (m_pending.isEmpty()) {
        QDBusConnection::sessionBus().connect(QString::fromLatin1(kService),
                                             QString::fromLatin1(kPath),
                                             QString::fromLatin1(kInterface),
                                             QStringLiteral("PathStatesChanged"),
                                             this,
                                             SLOT(onPathStatesChanged(QStringList)));
    }

    m_pending.insert(absolutePath);
    auto *watcher = new QDBusPendingCallWatcher(
        iface.asyncCall(QStringLiteral("GetPathStatesJson"), QStringList{absolutePath}),
        this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, absolutePath, watcher]() {
        QDBusPendingReply<QString> reply = *watcher;
        watcher->deleteLater();
        m_pending.remove(absolutePath);
        if (!reply.isValid()) {
            return;
        }

        QStringList overlays;
        const QJsonDocument document = QJsonDocument::fromJson(reply.value().toUtf8());
        if (document.isArray() && !document.array().isEmpty() && document.array().first().isObject()) {
            overlays = overlaysForState(document.array().first().toObject().value(QStringLiteral("state")).toString());
        }

        if (m_cache.value(absolutePath) != overlays) {
            m_cache.insert(absolutePath, overlays);
            emit overlaysChanged(QUrl::fromLocalFile(absolutePath), overlays);
        }
    });
}

QJsonObject OpenOneDriveOverlayIconPlugin::currentStatusObject() const
{
    const qint64 now = QDateTime::currentMSecsSinceEpoch();
    if (!m_statusCache.isEmpty() && now - m_statusCacheAtMs <= kStatusCacheTtlMs) {
        return m_statusCache;
    }

    QDBusInterface iface(QString::fromLatin1(kService),
                         QString::fromLatin1(kPath),
                         QString::fromLatin1(kInterface),
                         QDBusConnection::sessionBus());
    const QDBusReply<QString> reply = iface.call(QStringLiteral("GetStatusJson"));
    if (!reply.isValid()) {
        m_statusCache = {};
        m_statusCacheAtMs = now;
        return {};
    }

    const QJsonDocument document = QJsonDocument::fromJson(reply.value().toUtf8());
    if (!document.isObject()) {
        m_statusCache = {};
        m_statusCacheAtMs = now;
        return {};
    }

    m_statusCache = document.object();
    m_statusCacheAtMs = now;
    return m_statusCache;
}

QString OpenOneDriveOverlayIconPlugin::currentMountRoot() const
{
    return QDir::cleanPath(currentStatusObject().value(QStringLiteral("root_path")).toString());
}

QString OpenOneDriveOverlayIconPlugin::currentBackingDirName() const
{
    return currentStatusObject().value(QStringLiteral("backing_dir_name")).toString();
}

QStringList OpenOneDriveOverlayIconPlugin::overlaysForState(const QString &state)
{
    if (state == QStringLiteral("PinnedLocal")) {
        return {QStringLiteral("open-onedrive-pinned")};
    }
    if (state == QStringLiteral("AvailableLocal")) {
        return {QStringLiteral("open-onedrive-local")};
    }
    if (state == QStringLiteral("Syncing")) {
        return {QStringLiteral("open-onedrive-syncing")};
    }
    if (state == QStringLiteral("Error")) {
        return {QStringLiteral("open-onedrive-attention")};
    }
    if (state == QStringLiteral("Conflict")) {
        return {QStringLiteral("open-onedrive-attention")};
    }
    if (state == QStringLiteral("OnlineOnly")) {
        return {QStringLiteral("open-onedrive-online-only")};
    }
    return {};
}
