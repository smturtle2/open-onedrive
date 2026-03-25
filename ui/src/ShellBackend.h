#pragma once

#include <QObject>
#include <QString>

class QTimer;

class ShellBackend : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool configured READ configured NOTIFY configuredChanged)
    Q_PROPERTY(QString clientId READ clientId WRITE setClientId NOTIFY clientIdChanged)
    Q_PROPERTY(QString mountPath READ mountPath WRITE setMountPath NOTIFY mountPathChanged)
    Q_PROPERTY(QString syncState READ syncState NOTIFY syncStateChanged)
    Q_PROPERTY(QString mountState READ mountState NOTIFY mountStateChanged)
    Q_PROPERTY(QString statusMessage READ statusMessage NOTIFY statusMessageChanged)
    Q_PROPERTY(QString cacheUsageLabel READ cacheUsageLabel NOTIFY cacheUsageLabelChanged)
    Q_PROPERTY(QString indexedItemsLabel READ indexedItemsLabel NOTIFY indexedItemsLabelChanged)

public:
    explicit ShellBackend(QObject *parent = nullptr);

    bool configured() const;
    QString clientId() const;
    QString mountPath() const;
    QString syncState() const;
    QString mountState() const;
    QString statusMessage() const;
    QString cacheUsageLabel() const;
    QString indexedItemsLabel() const;

    void setClientId(const QString &clientId);
    void setMountPath(const QString &mountPath);

    Q_INVOKABLE void completeSetup();
    Q_INVOKABLE void pauseSync();
    Q_INVOKABLE void resumeSync();
    Q_INVOKABLE void openMountLocation();
    Q_INVOKABLE void freeUpSpace();
    Q_INVOKABLE void refreshStatus();

Q_SIGNALS:
    void configuredChanged();
    void clientIdChanged();
    void mountPathChanged();
    void syncStateChanged();
    void mountStateChanged();
    void statusMessageChanged();
    void cacheUsageLabelChanged();
    void indexedItemsLabelChanged();

private:
    void applyStatusJson(const QString &jsonPayload);
    void updateStatusMessage(const QString &message);
    void updateConfigured();

    QTimer *m_refreshTimer = nullptr;
    bool m_configured = false;
    QString m_clientId;
    QString m_mountPath;
    QString m_syncState = QStringLiteral("needs-setup");
    QString m_mountState = QStringLiteral("unmounted");
    QString m_statusMessage = QStringLiteral("Waiting for initial setup");
    QString m_cacheUsageLabel = QStringLiteral("Cache usage: pending daemon data");
    QString m_indexedItemsLabel = QStringLiteral("0 items indexed");
};
