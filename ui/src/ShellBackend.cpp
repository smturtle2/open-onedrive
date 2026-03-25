#include "ShellBackend.h"

#include <KStatusNotifierItem>

#include <QAction>
#include <QApplication>
#include <QClipboard>
#include <QCloseEvent>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusReply>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QEvent>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocale>
#include <QMenu>
#include <QMetaObject>
#include <QUrl>
#include <QWindow>

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

QString defaultMountPath()
{
    return QDir::cleanPath(QStringLiteral("%1/OneDrive").arg(qEnvironmentVariable("HOME")));
}
}

ShellBackend::ShellBackend(QObject *parent)
    : QObject(parent)
{
    m_mountPath = defaultMountPath();
    m_effectiveMountPath = m_mountPath;
    connectDaemonSignals();
    initializeTray();
    QMetaObject::invokeMethod(this, &ShellBackend::refreshStatus, Qt::QueuedConnection);
    QMetaObject::invokeMethod(this, &ShellBackend::refreshLogs, Qt::QueuedConnection);
}

bool ShellBackend::remoteConfigured() const
{
    return m_remoteConfigured;
}

bool ShellBackend::dashboardReady() const
{
    return m_remoteConfigured;
}

bool ShellBackend::customClientIdConfigured() const
{
    return m_customClientIdConfigured;
}

QString ShellBackend::mountPath() const
{
    return m_mountPath;
}

QString ShellBackend::effectiveMountPath() const
{
    return m_effectiveMountPath;
}

bool ShellBackend::mountPathPending() const
{
    return m_mountPath != m_effectiveMountPath;
}

QString ShellBackend::mountState() const
{
    return m_mountState;
}

QString ShellBackend::mountStateLabel() const
{
    if (m_mountState == QStringLiteral("Mounted")) {
        return tr("Mounted");
    }
    if (m_mountState == QStringLiteral("Mounting")) {
        return tr("Mounting");
    }
    if (m_mountState == QStringLiteral("Connecting")) {
        return tr("Connecting");
    }
    if (m_mountState == QStringLiteral("Unmounted")) {
        return tr("Ready to mount");
    }
    if (m_mountState == QStringLiteral("Error")) {
        return tr("Needs attention");
    }
    return tr("Disconnected");
}

QString ShellBackend::syncState() const
{
    return m_syncState;
}

QString ShellBackend::syncStateLabel() const
{
    if (m_syncState == QStringLiteral("Scanning")) {
        return tr("Scanning");
    }
    if (m_syncState == QStringLiteral("Syncing")) {
        return tr("Syncing");
    }
    if (m_syncState == QStringLiteral("Paused")) {
        return tr("Paused");
    }
    if (m_syncState == QStringLiteral("Error")) {
        return tr("Error");
    }
    return tr("Idle");
}

QString ShellBackend::statusMessage() const
{
    return m_statusMessage;
}

QString ShellBackend::cacheUsageLabel() const
{
    return m_cacheUsageLabel;
}

int ShellBackend::pinnedFileCount() const
{
    return m_pinnedFileCount;
}

int ShellBackend::queueDepth() const
{
    return m_queueDepth;
}

int ShellBackend::activeTransferCount() const
{
    return m_activeTransferCount;
}

QString ShellBackend::lastSyncLabel() const
{
    return formatTimestamp(m_lastSyncAt);
}

QString ShellBackend::lastSyncError() const
{
    return m_lastSyncError;
}

QString ShellBackend::rcloneVersion() const
{
    return m_rcloneVersion;
}

QString ShellBackend::lastLogLine() const
{
    return m_lastLogLine;
}

QStringList ShellBackend::recentLogs() const
{
    return m_recentLogs;
}

bool ShellBackend::canMount() const
{
    return m_remoteConfigured
           && m_mountState != QStringLiteral("Mounted")
           && m_mountState != QStringLiteral("Mounting")
           && m_mountState != QStringLiteral("Connecting");
}

bool ShellBackend::canUnmount() const
{
    return m_mountState == QStringLiteral("Mounted") || m_mountState == QStringLiteral("Mounting");
}

bool ShellBackend::canRetry() const
{
    return m_remoteConfigured
           && (m_mountState == QStringLiteral("Error")
               || m_mountState == QStringLiteral("Unmounted"));
}

bool ShellBackend::canPauseSync() const
{
    return m_remoteConfigured && m_syncState != QStringLiteral("Paused");
}

bool ShellBackend::canResumeSync() const
{
    return m_remoteConfigured && m_syncState == QStringLiteral("Paused");
}

void ShellBackend::setMountPath(const QString &mountPath)
{
    const QString normalizedPath = normalizeMountPath(mountPath);
    if (m_mountPath == normalizedPath) {
        return;
    }

    const bool pendingBefore = mountPathPending();
    m_mountPath = normalizedPath;
    emit mountPathChanged();
    if (pendingBefore != mountPathPending()) {
        emit mountPathPendingChanged();
    }
}

void ShellBackend::setMainWindow(QWindow *window)
{
    if (m_mainWindow == window) {
        return;
    }

    if (m_mainWindow != nullptr) {
        m_mainWindow->removeEventFilter(this);
    }
    m_mainWindow = window;
    if (m_mainWindow != nullptr) {
        m_mainWindow->installEventFilter(this);
    }

    if (m_tray != nullptr) {
        m_tray->setAssociatedWindow(m_mainWindow);
    }
    updateTray();
}

void ShellBackend::beginConnect()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus. Start openonedrived first."));
        return;
    }

    if (!syncMountPathIfNeeded(iface, tr("Choose a mount path before connecting."))) {
        return;
    }

    const QDBusReply<void> connectReply = iface.call(QStringLiteral("BeginConnect"));
    if (!connectReply.isValid()) {
        updateStatusMessage(tr("Connect failed: %1").arg(connectReply.error().message()));
        return;
    }

    updateStatusMessage(tr("Started the rclone browser sign-in flow. Finish the Microsoft login in your browser."));
    refreshStatus();
    refreshLogs();
}

void ShellBackend::disconnectRemote()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("Disconnect"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Disconnect failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
    refreshLogs();
}

void ShellBackend::mountRemote()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return;
    }

    if (!syncMountPathIfNeeded(iface, tr("Choose a mount path before mounting."))) {
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("Mount"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Mount failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::unmountRemote()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("Unmount"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Unmount failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::retryMount()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return;
    }

    if (!syncMountPathIfNeeded(iface, tr("Choose a mount path before retrying."))) {
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("RetryMount"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Retry failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::rescanRemote()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return;
    }

    const QDBusReply<uint> reply = iface.call(QStringLiteral("Rescan"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Rescan failed: %1").arg(reply.error().message()));
        return;
    }

    updateStatusMessage(tr("Rescanned %1 path state item(s).").arg(reply.value()));
    refreshStatus();
}

void ShellBackend::pauseSync()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("PauseSync"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Pause failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::resumeSync()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("ResumeSync"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Resume failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::openMountLocation()
{
    if (m_effectiveMountPath.isEmpty()) {
        updateStatusMessage(tr("Choose a mount path first."));
        return;
    }

    QDesktopServices::openUrl(QUrl::fromLocalFile(m_effectiveMountPath));
}

void ShellBackend::setMountPathFromUrl(const QUrl &mountPathUrl)
{
    if (!mountPathUrl.isValid()) {
        return;
    }

    setMountPath(mountPathUrl.isLocalFile() ? mountPathUrl.toLocalFile() : mountPathUrl.path());
}

QUrl ShellBackend::mountPathDialogFolder() const
{
    const QString candidatePath = !m_mountPath.isEmpty() ? m_mountPath : m_effectiveMountPath;
    if (candidatePath.isEmpty()) {
        return QUrl::fromLocalFile(QDir::homePath());
    }

    QFileInfo candidate(candidatePath);
    if (candidate.exists() && candidate.isDir()) {
        return QUrl::fromLocalFile(candidate.absoluteFilePath());
    }

    const QFileInfo parent(candidate.dir().absolutePath());
    if (parent.exists() && parent.isDir()) {
        return QUrl::fromLocalFile(parent.absoluteFilePath());
    }

    return QUrl::fromLocalFile(QDir::homePath());
}

void ShellBackend::keepLocalPath(const QString &path)
{
    if (invokePathAction(QStringLiteral("KeepLocal"),
                         path,
                         tr("Enter a path inside the mounted OneDrive folder."))) {
        refreshStatus();
        refreshLogs();
    }
}

void ShellBackend::makeOnlineOnlyPath(const QString &path)
{
    if (invokePathAction(QStringLiteral("MakeOnlineOnly"),
                         path,
                         tr("Enter a path inside the mounted OneDrive folder."))) {
        refreshStatus();
        refreshLogs();
    }
}

void ShellBackend::copyRecentLogsToClipboard()
{
    if (m_recentLogs.isEmpty()) {
        updateStatusMessage(tr("No recent logs to copy yet."));
        return;
    }

    if (QClipboard *clipboard = QGuiApplication::clipboard()) {
        clipboard->setText(m_recentLogs.join(QLatin1Char('\n')));
        updateStatusMessage(tr("Copied recent logs to the clipboard."));
    }
}

void ShellBackend::refreshStatus()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus. UI is waiting for the background service."));
        updateTray();
        return;
    }

    const QDBusReply<QString> reply = iface.call(QStringLiteral("GetStatusJson"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Status refresh failed: %1").arg(reply.error().message()));
        updateTray();
        return;
    }

    applyStatusJson(reply.value());
}

void ShellBackend::refreshLogs()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        return;
    }

    const QDBusReply<QStringList> reply = iface.call(QStringLiteral("GetRecentLogLines"), 100U);
    if (!reply.isValid()) {
        return;
    }

    if (reply.value() != m_recentLogs) {
        m_recentLogs = reply.value();
        emit recentLogsChanged();
    }
}

void ShellBackend::applyStatusJson(const QString &jsonPayload)
{
    const QJsonDocument document = QJsonDocument::fromJson(jsonPayload.toUtf8());
    if (!document.isObject()) {
        updateStatusMessage(tr("Daemon returned malformed status JSON."));
        return;
    }

    const QJsonObject object = document.object();
    const bool remoteConfigured = object.value(QStringLiteral("remote_configured")).toBool();
    const QString mountPath = object.value(QStringLiteral("mount_path")).toString();
    const QString mountState = object.value(QStringLiteral("mount_state")).toString();
    const QString syncState = object.value(QStringLiteral("sync_state")).toString();
    const QString lastError = object.value(QStringLiteral("last_error")).toString();
    const QString lastSyncError = object.value(QStringLiteral("last_sync_error")).toString();
    const QString lastLogLine = object.value(QStringLiteral("last_log_line")).toString();
    const QString rcloneVersion = object.value(QStringLiteral("rclone_version")).toString();
    const bool customClientIdConfigured = object.value(QStringLiteral("custom_client_id_configured")).toBool();
    const qint64 cacheBytes = object.value(QStringLiteral("cache_usage_bytes")).toInteger();
    const int pinnedFileCount = object.value(QStringLiteral("pinned_file_count")).toInt();
    const int queueDepth = object.value(QStringLiteral("queue_depth")).toInt();
    const int activeTransferCount = object.value(QStringLiteral("active_transfer_count")).toInt();
    const qint64 lastSyncAt = object.value(QStringLiteral("last_sync_at")).toInteger();

    const bool wasDashboardReady = dashboardReady();
    const bool pendingBefore = mountPathPending();
    const bool preserveDraftPath = pendingBefore;

    const QString normalizedMountPath = normalizeMountPath(mountPath);
    if (!normalizedMountPath.isEmpty() && normalizedMountPath != m_effectiveMountPath) {
        m_effectiveMountPath = normalizedMountPath;
        emit effectiveMountPathChanged();
    }

    if (!preserveDraftPath && !normalizedMountPath.isEmpty() && normalizedMountPath != m_mountPath) {
        m_mountPath = normalizedMountPath;
        emit mountPathChanged();
    }

    if (remoteConfigured != m_remoteConfigured) {
        m_remoteConfigured = remoteConfigured;
        emit remoteConfiguredChanged();
    }

    if (customClientIdConfigured != m_customClientIdConfigured) {
        m_customClientIdConfigured = customClientIdConfigured;
        emit customClientIdConfiguredChanged();
    }

    if (!mountState.isEmpty() && mountState != m_mountState) {
        m_mountState = mountState;
        emit mountStateChanged();
    }

    bool syncChanged = false;
    if (!syncState.isEmpty() && syncState != m_syncState) {
        m_syncState = syncState;
        syncChanged = true;
    }
    if (queueDepth != m_queueDepth) {
        m_queueDepth = queueDepth;
        syncChanged = true;
    }
    if (activeTransferCount != m_activeTransferCount) {
        m_activeTransferCount = activeTransferCount;
        syncChanged = true;
    }
    if (lastSyncAt != m_lastSyncAt) {
        m_lastSyncAt = lastSyncAt;
        syncChanged = true;
    }
    if (lastSyncError != m_lastSyncError) {
        m_lastSyncError = lastSyncError;
        syncChanged = true;
    }
    if (syncChanged) {
        emit syncStateChanged();
    }

    const QString cacheLabel = tr("%1 cached").arg(formatBytes(cacheBytes));
    if (cacheLabel != m_cacheUsageLabel) {
        m_cacheUsageLabel = cacheLabel;
        emit cacheUsageLabelChanged();
    }

    if (pinnedFileCount != m_pinnedFileCount) {
        m_pinnedFileCount = pinnedFileCount;
        emit pinnedFileCountChanged();
    }

    if (rcloneVersion != m_rcloneVersion) {
        m_rcloneVersion = rcloneVersion;
        emit rcloneVersionChanged();
    }

    if (lastLogLine != m_lastLogLine) {
        m_lastLogLine = lastLogLine;
        emit lastLogLineChanged();
    }

    if (pendingBefore != mountPathPending()) {
        emit mountPathPendingChanged();
    }

    if (wasDashboardReady != dashboardReady()) {
        emit dashboardReadyChanged();
    }

    if (!lastError.isEmpty()) {
        updateStatusMessage(lastError);
        updateTray();
        return;
    }

    if (!lastSyncError.isEmpty() && m_syncState == QStringLiteral("Error")) {
        updateStatusMessage(lastSyncError);
        updateTray();
        return;
    }

    if (!m_remoteConfigured) {
        updateStatusMessage(tr("Choose a mount folder, then start the OneDrive browser sign-in."));
        updateTray();
        return;
    }

    if (m_mountState == QStringLiteral("Connecting")) {
        updateStatusMessage(tr("Browser sign-in is in progress. Finish the Microsoft login flow."));
        updateTray();
        return;
    }

    if (m_mountState == QStringLiteral("Mounting")) {
        updateStatusMessage(tr("Starting rclone mount and waiting for the mountpoint to become ready."));
        updateTray();
        return;
    }

    if (m_syncState == QStringLiteral("Scanning")) {
        updateStatusMessage(tr("Refreshing remote state and updating Dolphin overlays."));
        updateTray();
        return;
    }

    if (m_syncState == QStringLiteral("Paused")) {
        updateStatusMessage(tr("Background sync is paused. Mounted files still stay available on demand."));
        updateTray();
        return;
    }

    if (m_syncState == QStringLiteral("Syncing")) {
        updateStatusMessage(tr("Applying on-device retention changes and syncing path state."));
        updateTray();
        return;
    }

    if (m_mountState == QStringLiteral("Mounted")) {
        updateStatusMessage(tr("rclone mount is active at %1.").arg(m_effectiveMountPath));
        updateTray();
        return;
    }

    if (m_mountState == QStringLiteral("Unmounted")) {
        updateStatusMessage(tr("OneDrive is configured but currently unmounted."));
        updateTray();
        return;
    }

    updateStatusMessage(tr("Review recent logs and reconnect if needed."));
    updateTray();
}

void ShellBackend::connectDaemonSignals()
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    bus.connect(QString::fromLatin1(kService),
                QString::fromLatin1(kPath),
                QString::fromLatin1(kInterface),
                QStringLiteral("MountStateChanged"),
                this,
                SLOT(onDaemonActivity()));
    bus.connect(QString::fromLatin1(kService),
                QString::fromLatin1(kPath),
                QString::fromLatin1(kInterface),
                QStringLiteral("SyncStateChanged"),
                this,
                SLOT(onDaemonActivity()));
    bus.connect(QString::fromLatin1(kService),
                QString::fromLatin1(kPath),
                QString::fromLatin1(kInterface),
                QStringLiteral("AuthFlowStarted"),
                this,
                SLOT(onDaemonActivity()));
    bus.connect(QString::fromLatin1(kService),
                QString::fromLatin1(kPath),
                QString::fromLatin1(kInterface),
                QStringLiteral("AuthFlowCompleted"),
                this,
                SLOT(onDaemonActivity()));
    bus.connect(QString::fromLatin1(kService),
                QString::fromLatin1(kPath),
                QString::fromLatin1(kInterface),
                QStringLiteral("LogsUpdated"),
                this,
                SLOT(onLogsUpdated()));
    bus.connect(QString::fromLatin1(kService),
                QString::fromLatin1(kPath),
                QString::fromLatin1(kInterface),
                QStringLiteral("PathStatesChanged"),
                this,
                SLOT(onDaemonActivity()));
    bus.connect(QString::fromLatin1(kService),
                QString::fromLatin1(kPath),
                QString::fromLatin1(kInterface),
                QStringLiteral("ErrorRaised"),
                this,
                SLOT(onErrorRaised(QString)));
}

void ShellBackend::initializeTray()
{
    if (m_tray != nullptr) {
        return;
    }

    m_tray = new KStatusNotifierItem(QStringLiteral("open-onedrive"), this);
    m_tray->setTitle(QStringLiteral("open-onedrive"));
    m_tray->setCategory(KStatusNotifierItem::ApplicationStatus);
    m_tray->setStandardActionsEnabled(false);
    m_tray->setIconByName(QStringLiteral("folder-cloud"));
    m_tray->setToolTip(QStringLiteral("folder-cloud"), QStringLiteral("open-onedrive"), m_statusMessage);

    m_trayMenu = new QMenu;
    m_showWindowAction = m_trayMenu->addAction(tr("Open Dashboard"));
    m_mountAction = m_trayMenu->addAction(tr("Mount"));
    m_unmountAction = m_trayMenu->addAction(tr("Unmount"));
    m_rescanAction = m_trayMenu->addAction(tr("Rescan"));
    m_pauseSyncAction = m_trayMenu->addAction(tr("Pause Sync"));
    m_resumeSyncAction = m_trayMenu->addAction(tr("Resume Sync"));
    m_trayMenu->addSeparator();
    m_quitAction = m_trayMenu->addAction(tr("Quit"));

    connect(m_showWindowAction, &QAction::triggered, this, [this]() {
        if (m_mainWindow == nullptr) {
            return;
        }
        m_mainWindow->show();
        m_mainWindow->raise();
        m_mainWindow->requestActivate();
    });
    connect(m_mountAction, &QAction::triggered, this, &ShellBackend::mountRemote);
    connect(m_unmountAction, &QAction::triggered, this, &ShellBackend::unmountRemote);
    connect(m_rescanAction, &QAction::triggered, this, &ShellBackend::rescanRemote);
    connect(m_pauseSyncAction, &QAction::triggered, this, &ShellBackend::pauseSync);
    connect(m_resumeSyncAction, &QAction::triggered, this, &ShellBackend::resumeSync);
    connect(m_quitAction, &QAction::triggered, this, [this]() {
        m_allowQuit = true;
        QCoreApplication::quit();
    });
    connect(m_tray, &KStatusNotifierItem::activateRequested, this, [this](bool, const QPoint &) {
        if (m_mainWindow == nullptr) {
            return;
        }
        if (m_mainWindow->isVisible()) {
            m_mainWindow->hide();
        } else {
            m_mainWindow->show();
            m_mainWindow->raise();
            m_mainWindow->requestActivate();
        }
        updateTray();
    });
    connect(m_tray, &KStatusNotifierItem::quitRequested, this, [this]() {
        m_allowQuit = true;
        QCoreApplication::quit();
    });

    m_tray->setContextMenu(m_trayMenu);
    if (m_mainWindow != nullptr) {
        m_tray->setAssociatedWindow(m_mainWindow);
    }
    updateTray();
}

bool ShellBackend::syncMountPathIfNeeded(QDBusInterface &iface, const QString &emptyPathMessage)
{
    if (m_mountPath.isEmpty()) {
        updateStatusMessage(emptyPathMessage);
        return false;
    }

    if (m_mountPath == m_effectiveMountPath) {
        return true;
    }

    const bool pendingBefore = mountPathPending();
    const QDBusReply<void> pathReply = iface.call(QStringLiteral("SetMountPath"), m_mountPath);
    if (!pathReply.isValid()) {
        updateStatusMessage(tr("Mount path update failed: %1").arg(pathReply.error().message()));
        return false;
    }

    m_effectiveMountPath = m_mountPath;
    emit effectiveMountPathChanged();
    if (pendingBefore != mountPathPending()) {
        emit mountPathPendingChanged();
    }
    return true;
}

void ShellBackend::updateStatusMessage(const QString &message)
{
    if (m_statusMessage == message) {
        return;
    }

    m_statusMessage = message;
    emit statusMessageChanged();
    updateTray();
}

void ShellBackend::updateTray()
{
    if (m_tray == nullptr) {
        return;
    }

    m_showWindowAction->setText(m_mainWindow != nullptr && m_mainWindow->isVisible()
                                    ? tr("Hide Window")
                                    : tr("Open Dashboard"));
    m_mountAction->setEnabled(canMount());
    m_unmountAction->setEnabled(canUnmount());
    m_rescanAction->setEnabled(m_remoteConfigured);
    m_pauseSyncAction->setEnabled(canPauseSync());
    m_resumeSyncAction->setEnabled(canResumeSync());

    auto trayStatus = KStatusNotifierItem::Active;
    QString overlayIcon;
    if (m_mountState == QStringLiteral("Error") || m_syncState == QStringLiteral("Error")) {
        trayStatus = KStatusNotifierItem::NeedsAttention;
        overlayIcon = QStringLiteral("emblem-important");
    } else if (m_mountState == QStringLiteral("Connecting")
               || m_mountState == QStringLiteral("Mounting")
               || m_syncState == QStringLiteral("Scanning")
               || m_syncState == QStringLiteral("Syncing")) {
        overlayIcon = QStringLiteral("emblem-synchronizing");
    } else if (m_syncState == QStringLiteral("Paused")) {
        overlayIcon = QStringLiteral("media-playback-pause");
    } else if (!m_remoteConfigured) {
        trayStatus = KStatusNotifierItem::Passive;
    } else if (m_mountState == QStringLiteral("Mounted")) {
        overlayIcon = QStringLiteral("emblem-checked");
    }

    m_tray->setStatus(trayStatus);
    m_tray->setIconByName(QStringLiteral("folder-cloud"));
    m_tray->setOverlayIconByName(overlayIcon);
    m_tray->setToolTip(QStringLiteral("folder-cloud"), QStringLiteral("open-onedrive"), m_statusMessage);
}

bool ShellBackend::invokePathAction(const QString &method,
                                    const QString &path,
                                    const QString &emptyPathMessage)
{
    const QString normalizedPath = normalizeMountPath(path);
    if (normalizedPath.isEmpty()) {
        updateStatusMessage(emptyPathMessage);
        return false;
    }

    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return false;
    }

    const QDBusReply<uint> reply = iface.call(method, QStringList{normalizedPath});
    if (!reply.isValid()) {
        updateStatusMessage(tr("%1 failed: %2").arg(method, reply.error().message()));
        return false;
    }

    updateStatusMessage(tr("%1 applied to %2 item(s).").arg(method, QString::number(reply.value())));
    return true;
}

QString ShellBackend::normalizeMountPath(const QString &mountPath)
{
    const QString trimmedPath = mountPath.trimmed();
    if (trimmedPath.isEmpty()) {
        return QString();
    }
    return QDir::cleanPath(trimmedPath);
}

QString ShellBackend::formatBytes(qint64 bytes)
{
    return QLocale().formattedDataSize(bytes);
}

QString ShellBackend::formatTimestamp(qint64 secondsSinceEpoch)
{
    if (secondsSinceEpoch <= 0) {
        return QObject::tr("Not yet");
    }

    return QLocale().toString(QDateTime::fromSecsSinceEpoch(secondsSinceEpoch).toLocalTime(),
                              QLocale::ShortFormat);
}

void ShellBackend::onDaemonActivity()
{
    refreshStatus();
}

void ShellBackend::onLogsUpdated()
{
    refreshLogs();
}

void ShellBackend::onErrorRaised(const QString &message)
{
    if (m_tray != nullptr) {
        m_tray->showMessage(QStringLiteral("open-onedrive"),
                            message,
                            QStringLiteral("dialog-error"),
                            10000);
    }
    refreshStatus();
    refreshLogs();
}

bool ShellBackend::eventFilter(QObject *watched, QEvent *event)
{
    if (watched == m_mainWindow && event->type() == QEvent::Close && !m_allowQuit) {
        if (m_mainWindow != nullptr) {
            m_mainWindow->hide();
        }
        updateStatusMessage(tr("open-onedrive is still running in the system tray."));
        event->ignore();
        updateTray();
        return true;
    }

    return QObject::eventFilter(watched, event);
}
