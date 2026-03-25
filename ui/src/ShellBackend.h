#pragma once

#include <QObject>
#include <QString>
#include <QStringList>
#include <QUrl>

class QTimer;
class QDBusInterface;
class QAction;
class QMenu;
class QWindow;
class KStatusNotifierItem;

class ShellBackend : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool remoteConfigured READ remoteConfigured NOTIFY remoteConfiguredChanged)
    Q_PROPERTY(bool dashboardReady READ dashboardReady NOTIFY dashboardReadyChanged)
    Q_PROPERTY(bool customClientIdConfigured READ customClientIdConfigured NOTIFY customClientIdConfiguredChanged)
    Q_PROPERTY(QString connectionState READ connectionState NOTIFY connectionStateChanged)
    Q_PROPERTY(QString connectionStateLabel READ connectionStateLabel NOTIFY connectionStateChanged)
    Q_PROPERTY(QString mountPath READ mountPath WRITE setMountPath NOTIFY mountPathChanged)
    Q_PROPERTY(QString effectiveMountPath READ effectiveMountPath NOTIFY effectiveMountPathChanged)
    Q_PROPERTY(bool mountPathPending READ mountPathPending NOTIFY mountPathPendingChanged)
    Q_PROPERTY(QString mountState READ mountState NOTIFY mountStateChanged)
    Q_PROPERTY(QString mountStateLabel READ mountStateLabel NOTIFY mountStateChanged)
    Q_PROPERTY(QString syncState READ syncState NOTIFY syncStateChanged)
    Q_PROPERTY(QString syncStateLabel READ syncStateLabel NOTIFY syncStateChanged)
    Q_PROPERTY(QString statusMessage READ statusMessage NOTIFY statusMessageChanged)
    Q_PROPERTY(QString cacheUsageLabel READ cacheUsageLabel NOTIFY cacheUsageLabelChanged)
    Q_PROPERTY(QString backingDirName READ backingDirName NOTIFY backingDirNameChanged)
    Q_PROPERTY(int pinnedFileCount READ pinnedFileCount NOTIFY pinnedFileCountChanged)
    Q_PROPERTY(int pendingDownloads READ pendingDownloads NOTIFY syncStateChanged)
    Q_PROPERTY(int pendingUploads READ pendingUploads NOTIFY syncStateChanged)
    Q_PROPERTY(int conflictCount READ conflictCount NOTIFY syncStateChanged)
    Q_PROPERTY(int queueDepth READ queueDepth NOTIFY syncStateChanged)
    Q_PROPERTY(int activeTransferCount READ activeTransferCount NOTIFY syncStateChanged)
    Q_PROPERTY(QString lastSyncLabel READ lastSyncLabel NOTIFY syncStateChanged)
    Q_PROPERTY(QString lastSyncError READ lastSyncError NOTIFY syncStateChanged)
    Q_PROPERTY(QString rcloneVersion READ rcloneVersion NOTIFY rcloneVersionChanged)
    Q_PROPERTY(QString lastLogLine READ lastLogLine NOTIFY lastLogLineChanged)
    Q_PROPERTY(QStringList recentLogs READ recentLogs NOTIFY recentLogsChanged)
    Q_PROPERTY(bool canMount READ canMount NOTIFY mountStateChanged)
    Q_PROPERTY(bool canUnmount READ canUnmount NOTIFY mountStateChanged)
    Q_PROPERTY(bool canRetry READ canRetry NOTIFY mountStateChanged)
    Q_PROPERTY(bool canPauseSync READ canPauseSync NOTIFY syncStateChanged)
    Q_PROPERTY(bool canResumeSync READ canResumeSync NOTIFY syncStateChanged)

public:
    explicit ShellBackend(QObject *parent = nullptr);

    bool remoteConfigured() const;
    bool dashboardReady() const;
    bool customClientIdConfigured() const;
    QString connectionState() const;
    QString connectionStateLabel() const;
    QString mountPath() const;
    QString effectiveMountPath() const;
    bool mountPathPending() const;
    QString mountState() const;
    QString mountStateLabel() const;
    QString syncState() const;
    QString syncStateLabel() const;
    QString statusMessage() const;
    QString cacheUsageLabel() const;
    QString backingDirName() const;
    int pinnedFileCount() const;
    int pendingDownloads() const;
    int pendingUploads() const;
    int conflictCount() const;
    int queueDepth() const;
    int activeTransferCount() const;
    QString lastSyncLabel() const;
    QString lastSyncError() const;
    QString rcloneVersion() const;
    QString lastLogLine() const;
    QStringList recentLogs() const;
    bool canMount() const;
    bool canUnmount() const;
    bool canRetry() const;
    bool canPauseSync() const;
    bool canResumeSync() const;

    void setMountPath(const QString &mountPath);
    void setMainWindow(QWindow *window);

    Q_INVOKABLE void beginConnect();
    Q_INVOKABLE void disconnectRemote();
    Q_INVOKABLE void mountRemote();
    Q_INVOKABLE void unmountRemote();
    Q_INVOKABLE void retryMount();
    Q_INVOKABLE void retryTransferPath(const QString &path);
    Q_INVOKABLE void rescanRemote();
    Q_INVOKABLE void pauseSync();
    Q_INVOKABLE void resumeSync();
    Q_INVOKABLE void openMountLocation();
    Q_INVOKABLE void setMountPathFromUrl(const QUrl &mountPathUrl);
    Q_INVOKABLE QUrl mountPathDialogFolder() const;
    Q_INVOKABLE void keepLocalPath(const QString &path);
    Q_INVOKABLE void makeOnlineOnlyPath(const QString &path);
    Q_INVOKABLE void copyRecentLogsToClipboard();
    Q_INVOKABLE void refreshStatus();
    Q_INVOKABLE void refreshLogs();

Q_SIGNALS:
    void remoteConfiguredChanged();
    void dashboardReadyChanged();
    void customClientIdConfiguredChanged();
    void connectionStateChanged();
    void mountPathChanged();
    void effectiveMountPathChanged();
    void mountPathPendingChanged();
    void mountStateChanged();
    void syncStateChanged();
    void statusMessageChanged();
    void cacheUsageLabelChanged();
    void backingDirNameChanged();
    void pinnedFileCountChanged();
    void rcloneVersionChanged();
    void lastLogLineChanged();
    void recentLogsChanged();

protected:
    bool eventFilter(QObject *watched, QEvent *event) override;

private:
    void applyStatusJson(const QString &jsonPayload);
    void connectDaemonSignals();
    void initializeTray();
    bool syncMountPathIfNeeded(QDBusInterface &iface, const QString &emptyPathMessage);
    void updateStatusMessage(const QString &message);
    void updateTray();
    bool invokePathAction(const QString &method, const QString &path, const QString &emptyPathMessage);
    static QString normalizeMountPath(const QString &mountPath);
    static QString formatBytes(qint64 bytes);
    static QString formatTimestamp(qint64 secondsSinceEpoch);

private Q_SLOTS:
    void onDaemonActivity();
    void onLogsUpdated();
    void onErrorRaised(const QString &message);

private:
    bool m_remoteConfigured = false;
    bool m_customClientIdConfigured = false;
    QString m_connectionState = QStringLiteral("Disconnected");
    QString m_mountPath;
    QString m_effectiveMountPath;
    QString m_mountState = QStringLiteral("Stopped");
    QString m_syncState = QStringLiteral("Idle");
    QString m_statusMessage = QStringLiteral("Choose a OneDrive root folder, then start the browser sign-in.");
    QString m_cacheUsageLabel = QStringLiteral("Backing store usage: pending daemon data");
    QString m_backingDirName = QStringLiteral(".openonedrive-cache");
    int m_pinnedFileCount = 0;
    int m_pendingDownloads = 0;
    int m_pendingUploads = 0;
    int m_conflictCount = 0;
    int m_queueDepth = 0;
    int m_activeTransferCount = 0;
    qint64 m_lastSyncAt = 0;
    QString m_lastSyncError;
    QString m_rcloneVersion;
    QString m_lastLogLine;
    QStringList m_recentLogs;
    QWindow *m_mainWindow = nullptr;
    KStatusNotifierItem *m_tray = nullptr;
    QMenu *m_trayMenu = nullptr;
    QAction *m_showWindowAction = nullptr;
    QAction *m_mountAction = nullptr;
    QAction *m_unmountAction = nullptr;
    QAction *m_rescanAction = nullptr;
    QAction *m_pauseSyncAction = nullptr;
    QAction *m_resumeSyncAction = nullptr;
    QAction *m_quitAction = nullptr;
    bool m_allowQuit = false;
};
