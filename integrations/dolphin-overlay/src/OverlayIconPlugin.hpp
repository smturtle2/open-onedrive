#pragma once

#include <KOverlayIconPlugin>

#include <QHash>
#include <QJsonObject>
#include <QSet>
#include <QStringList>

class OpenOneDriveOverlayIconPlugin : public KOverlayIconPlugin
{
    Q_OBJECT
    Q_PLUGIN_METADATA(IID "org.kde.overlayicon.open_onedrive")

public:
    using KOverlayIconPlugin::KOverlayIconPlugin;

    QStringList getOverlays(const QUrl &item) override;

private Q_SLOTS:
    void onPathStatesChanged(const QStringList &paths);

private:
    void requestPathState(const QString &absolutePath);
    QJsonObject currentStatusObject() const;
    QString currentMountRoot() const;
    QString currentBackingDirName() const;
    static QStringList overlaysForState(const QString &state);

    mutable QJsonObject m_statusCache;
    mutable qint64 m_statusCacheAtMs = 0;
    QHash<QString, QStringList> m_cache;
    QSet<QString> m_pending;
};
