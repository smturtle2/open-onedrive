#pragma once

#include <QObject>
#include <QString>
#include <QStringList>
#include <QUrl>

class QTimer;
class QDBusInterface;

class ShellBackend : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool remoteConfigured READ remoteConfigured NOTIFY remoteConfiguredChanged)
    Q_PROPERTY(bool dashboardReady READ dashboardReady NOTIFY dashboardReadyChanged)
    Q_PROPERTY(bool customClientIdConfigured READ customClientIdConfigured NOTIFY customClientIdConfiguredChanged)
    Q_PROPERTY(QString mountPath READ mountPath WRITE setMountPath NOTIFY mountPathChanged)
    Q_PROPERTY(QString effectiveMountPath READ effectiveMountPath NOTIFY effectiveMountPathChanged)
    Q_PROPERTY(bool mountPathPending READ mountPathPending NOTIFY mountPathPendingChanged)
    Q_PROPERTY(QString mountState READ mountState NOTIFY mountStateChanged)
    Q_PROPERTY(QString mountStateLabel READ mountStateLabel NOTIFY mountStateChanged)
    Q_PROPERTY(QString statusMessage READ statusMessage NOTIFY statusMessageChanged)
    Q_PROPERTY(QString cacheUsageLabel READ cacheUsageLabel NOTIFY cacheUsageLabelChanged)
    Q_PROPERTY(int pinnedFileCount READ pinnedFileCount NOTIFY pinnedFileCountChanged)
    Q_PROPERTY(QString rcloneVersion READ rcloneVersion NOTIFY rcloneVersionChanged)
    Q_PROPERTY(QString lastLogLine READ lastLogLine NOTIFY lastLogLineChanged)
    Q_PROPERTY(QStringList recentLogs READ recentLogs NOTIFY recentLogsChanged)
    Q_PROPERTY(bool canMount READ canMount NOTIFY mountStateChanged)
    Q_PROPERTY(bool canUnmount READ canUnmount NOTIFY mountStateChanged)
    Q_PROPERTY(bool canRetry READ canRetry NOTIFY mountStateChanged)

public:
    explicit ShellBackend(QObject *parent = nullptr);

    bool remoteConfigured() const;
    bool dashboardReady() const;
    bool customClientIdConfigured() const;
    QString mountPath() const;
    QString effectiveMountPath() const;
    bool mountPathPending() const;
    QString mountState() const;
    QString mountStateLabel() const;
    QString statusMessage() const;
    QString cacheUsageLabel() const;
    int pinnedFileCount() const;
    QString rcloneVersion() const;
    QString lastLogLine() const;
    QStringList recentLogs() const;
    bool canMount() const;
    bool canUnmount() const;
    bool canRetry() const;

    void setMountPath(const QString &mountPath);

    Q_INVOKABLE void beginConnect();
    Q_INVOKABLE void disconnectRemote();
    Q_INVOKABLE void mountRemote();
    Q_INVOKABLE void unmountRemote();
    Q_INVOKABLE void retryMount();
    Q_INVOKABLE void openMountLocation();
    Q_INVOKABLE void setMountPathFromUrl(const QUrl &mountPathUrl);
    Q_INVOKABLE QUrl mountPathDialogFolder() const;
    Q_INVOKABLE void copyRecentLogsToClipboard();
    Q_INVOKABLE void refreshStatus();
    Q_INVOKABLE void refreshLogs();

Q_SIGNALS:
    void remoteConfiguredChanged();
    void dashboardReadyChanged();
    void customClientIdConfiguredChanged();
    void mountPathChanged();
    void effectiveMountPathChanged();
    void mountPathPendingChanged();
    void mountStateChanged();
    void statusMessageChanged();
    void cacheUsageLabelChanged();
    void pinnedFileCountChanged();
    void rcloneVersionChanged();
    void lastLogLineChanged();
    void recentLogsChanged();

private:
    void applyStatusJson(const QString &jsonPayload);
    bool syncMountPathIfNeeded(QDBusInterface &iface, const QString &emptyPathMessage);
    void updateStatusMessage(const QString &message);
    static QString normalizeMountPath(const QString &mountPath);
    static QString formatBytes(qint64 bytes);

    QTimer *m_refreshTimer = nullptr;
    bool m_remoteConfigured = false;
    bool m_customClientIdConfigured = false;
    QString m_mountPath;
    QString m_effectiveMountPath;
    QString m_mountState = QStringLiteral("Disconnected");
    QString m_statusMessage = QStringLiteral("Choose a mount folder, then start the OneDrive browser sign-in.");
    QString m_cacheUsageLabel = QStringLiteral("Cache usage: pending daemon data");
    int m_pinnedFileCount = 0;
    QString m_rcloneVersion;
    QString m_lastLogLine;
    QStringList m_recentLogs;
};
