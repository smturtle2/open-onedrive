#include "ShellBackend.h"

#include <KStatusNotifierItem>

#include <QAction>
#include <QApplication>
#include <QClipboard>
#include <QCloseEvent>
#include <QDBusConnection>
#include <QDBusError>
#include <QDBusInterface>
#include <QDBusReply>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QEvent>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonArray>
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

bool isDaemonUnavailableError(const QDBusError &error)
{
    switch (error.type()) {
    case QDBusError::ServiceUnknown:
    case QDBusError::NoReply:
    case QDBusError::Disconnected:
    case QDBusError::UnknownObject:
    case QDBusError::UnknownInterface:
        return true;
    default:
        return false;
    }
}

QString actionLabelForMethod(const QString &method)
{
    if (method == QStringLiteral("KeepLocal")) {
        return QObject::tr("Keep on device");
    }
    if (method == QStringLiteral("MakeOnlineOnly")) {
        return QObject::tr("Make online-only");
    }
    if (method == QStringLiteral("RetryTransfer")) {
        return QObject::tr("Retry transfer");
    }
    return method;
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

bool ShellBackend::needsRemoteRepair() const
{
    return m_needsRemoteRepair;
}

bool ShellBackend::dashboardReady() const
{
    return m_daemonReachable && m_remoteConfigured;
}

bool ShellBackend::daemonReachable() const
{
    return m_daemonReachable;
}

QString ShellBackend::appState() const
{
    if (!m_daemonReachable) {
        return QStringLiteral("daemon-unavailable");
    }
    if (!m_remoteConfigured) {
        return QStringLiteral("welcome");
    }
    if (m_connectionState == QStringLiteral("Connecting")) {
        return QStringLiteral("connecting");
    }
    if (m_mountState == QStringLiteral("Starting")
        || m_syncState == QStringLiteral("Scanning")
        || m_syncState == QStringLiteral("Syncing")) {
        return QStringLiteral("connecting");
    }
    if (m_connectionState == QStringLiteral("Error")
        || m_needsRemoteRepair
        || m_mountState == QStringLiteral("Error")
        || m_mountState == QStringLiteral("Degraded")
        || m_syncState == QStringLiteral("Error")
        || m_conflictCount > 0) {
        return QStringLiteral("recovery");
    }
    if (m_mountState == QStringLiteral("Running")) {
        return QStringLiteral("running");
    }
    return QStringLiteral("ready");
}

bool ShellBackend::customClientIdConfigured() const
{
    return m_customClientIdConfigured;
}

QString ShellBackend::connectionState() const
{
    return m_connectionState;
}

QString ShellBackend::connectionStateLabel() const
{
    if (m_connectionState == QStringLiteral("Connecting")) {
        return tr("Connecting");
    }
    if (m_connectionState == QStringLiteral("Ready")) {
        return tr("Ready");
    }
    if (m_connectionState == QStringLiteral("Error")) {
        return tr("Needs attention");
    }
    return tr("Disconnected");
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
    if (m_mountState == QStringLiteral("Running")) {
        return tr("Running");
    }
    if (m_mountState == QStringLiteral("Starting")) {
        return tr("Starting");
    }
    if (m_mountState == QStringLiteral("Stopped")) {
        return tr("Stopped");
    }
    if (m_mountState == QStringLiteral("Error")) {
        return tr("Needs attention");
    }
    if (m_mountState == QStringLiteral("Degraded")) {
        return tr("Degraded");
    }
    return tr("Unknown");
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

QString ShellBackend::backingDirName() const
{
    return m_backingDirName;
}

int ShellBackend::pinnedFileCount() const
{
    return m_pinnedFileCount;
}

int ShellBackend::pendingDownloads() const
{
    return m_pendingDownloads;
}

int ShellBackend::pendingUploads() const
{
    return m_pendingUploads;
}

int ShellBackend::conflictCount() const
{
    return m_conflictCount;
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

QVariantList ShellBackend::recentLogEntries() const
{
    return m_recentLogEntries;
}

bool ShellBackend::canMount() const
{
    return m_daemonReachable
           && m_remoteConfigured
           && !m_needsRemoteRepair
           && m_connectionState == QStringLiteral("Ready")
           && m_mountState != QStringLiteral("Running")
           && m_mountState != QStringLiteral("Starting");
}

bool ShellBackend::canUnmount() const
{
    return m_daemonReachable
           && (m_mountState == QStringLiteral("Running")
               || m_mountState == QStringLiteral("Starting"));
}

bool ShellBackend::canRetry() const
{
    return m_daemonReachable
           && m_remoteConfigured
           && !m_needsRemoteRepair
           && (m_mountState == QStringLiteral("Error")
               || m_mountState == QStringLiteral("Stopped")
               || m_connectionState == QStringLiteral("Error"));
}

bool ShellBackend::canPauseSync() const
{
    return m_daemonReachable && m_remoteConfigured && m_syncState != QStringLiteral("Paused");
}

bool ShellBackend::canResumeSync() const
{
    return m_daemonReachable && m_remoteConfigured && m_syncState == QStringLiteral("Paused");
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

void ShellBackend::activateMainWindow()
{
    if (m_mainWindow == nullptr) {
        return;
    }
    m_mainWindow->show();
    m_mainWindow->raise();
    m_mainWindow->requestActivate();
    updateTray();
}

void ShellBackend::beginConnect()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus. Start openonedrived first."));
        return;
    }

    if (!syncMountPathIfNeeded(iface, tr("Choose a root folder before connecting."))) {
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

void ShellBackend::repairRemote()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return;
    }

    if (!syncMountPathIfNeeded(iface, tr("Choose a root folder before repairing the OneDrive profile."))) {
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("RepairRemote"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Repair failed: %1").arg(reply.error().message()));
        return;
    }

    updateStatusMessage(tr("Repair cleared the stale rclone profile and restarted the browser sign-in flow. Finish the Microsoft login in your browser."));
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

    if (!syncMountPathIfNeeded(iface, tr("Choose a root folder before starting the filesystem."))) {
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("StartFilesystem"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Start filesystem failed: %1").arg(reply.error().message()));
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

    const QDBusReply<void> reply = iface.call(QStringLiteral("StopFilesystem"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Stop filesystem failed: %1").arg(reply.error().message()));
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

    if (!syncMountPathIfNeeded(iface, tr("Choose a root folder before retrying."))) {
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("RetryFilesystem"));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Retry failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::retryTransferPath(const QString &path)
{
    if (invokePathAction(QStringLiteral("RetryTransfer"),
                         path,
                         tr("Enter a path inside the OneDrive root folder."))) {
        refreshStatus();
        refreshLogs();
    }
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
        updateStatusMessage(tr("Choose a root folder first."));
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
                         tr("Enter a path inside the OneDrive root folder."))) {
        refreshStatus();
        refreshLogs();
    }
}

void ShellBackend::keepLocalPaths(const QStringList &paths)
{
    if (invokePathsAction(QStringLiteral("KeepLocal"),
                          paths,
                          tr("Select at least one path inside the OneDrive root folder."))) {
        refreshStatus();
        refreshLogs();
    }
}

void ShellBackend::makeOnlineOnlyPath(const QString &path)
{
    if (invokePathAction(QStringLiteral("MakeOnlineOnly"),
                         path,
                         tr("Enter a path inside the OneDrive root folder."))) {
        refreshStatus();
        refreshLogs();
    }
}

void ShellBackend::makeOnlineOnlyPaths(const QStringList &paths)
{
    if (invokePathsAction(QStringLiteral("MakeOnlineOnly"),
                          paths,
                          tr("Select at least one path inside the OneDrive root folder."))) {
        refreshStatus();
        refreshLogs();
    }
}

void ShellBackend::retryTransferPaths(const QStringList &paths)
{
    if (invokePathsAction(QStringLiteral("RetryTransfer"),
                          paths,
                          tr("Select at least one path inside the OneDrive root folder."))) {
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

void ShellBackend::copyLinesToClipboard(const QStringList &lines)
{
    if (lines.isEmpty()) {
        updateStatusMessage(tr("No log lines to copy."));
        return;
    }

    if (QClipboard *clipboard = QGuiApplication::clipboard()) {
        clipboard->setText(lines.join(QLatin1Char('\n')));
        updateStatusMessage(tr("Copied the selected log lines to the clipboard."));
    }
}

QString ShellBackend::listDirectoryJson(const QString &path)
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return QStringLiteral("[]");
    }

    const QDBusReply<QString> reply = iface.call(QStringLiteral("ListDirectoryJson"), path);
    if (!reply.isValid()) {
        updateStatusMessage(tr("Directory listing failed: %1").arg(reply.error().message()));
        return QStringLiteral("[]");
    }

    return reply.value();
}

QString ShellBackend::searchPathsJson(const QString &query, int limit)
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return QStringLiteral("[]");
    }

    const QDBusReply<QString> reply = iface.call(QStringLiteral("SearchPathsJson"),
                                                 query,
                                                 qMax(0, limit));
    if (!reply.isValid()) {
        updateStatusMessage(tr("Search failed: %1").arg(reply.error().message()));
        return QStringLiteral("[]");
    }

    return reply.value();
}

void ShellBackend::openPath(const QString &path)
{
    const QString normalizedPath = normalizeMountPath(path);
    if (normalizedPath.isEmpty()) {
        updateStatusMessage(tr("Choose a path inside the OneDrive root folder first."));
        return;
    }

    if (!QFileInfo(normalizedPath).isAbsolute() && m_effectiveMountPath.isEmpty()) {
        updateStatusMessage(tr("Choose a root folder first."));
        return;
    }

    const QString absolutePath = QFileInfo(normalizedPath).isAbsolute()
                                     ? normalizedPath
                                     : QDir(m_effectiveMountPath).filePath(normalizedPath);
    QDesktopServices::openUrl(QUrl::fromLocalFile(absolutePath));
}

void ShellBackend::refreshStatus()
{
    const QString previousAppState = appState();
    const bool wasDashboardReady = dashboardReady();
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        if (m_daemonReachable) {
            m_daemonReachable = false;
            emit daemonReachableChanged();
            emit mountStateChanged();
            emit syncStateChanged();
        }
        updateStatusMessage(
            tr("Background service unavailable. Start openonedrived or run systemctl --user start openonedrived.service, then refresh here."));
        if (previousAppState != appState()) {
            emit appStateChanged();
        }
        if (wasDashboardReady != dashboardReady()) {
            emit dashboardReadyChanged();
        }
        updateTray();
        return;
    }

    const QDBusReply<QString> reply = iface.call(QStringLiteral("GetStatusJson"));
    if (!reply.isValid()) {
        const QDBusError error = reply.error();
        if (isDaemonUnavailableError(error)) {
            if (m_daemonReachable) {
                m_daemonReachable = false;
                emit daemonReachableChanged();
                emit mountStateChanged();
                emit syncStateChanged();
            }
            updateStatusMessage(
                tr("Background service unavailable. Start openonedrived or run systemctl --user start openonedrived.service, then refresh here."));
        } else {
            if (!m_daemonReachable) {
                m_daemonReachable = true;
                emit daemonReachableChanged();
                emit mountStateChanged();
                emit syncStateChanged();
            }
            updateStatusMessage(tr("Daemon status refresh failed: %1").arg(error.message()));
        }
        if (previousAppState != appState()) {
            emit appStateChanged();
        }
        if (wasDashboardReady != dashboardReady()) {
            emit dashboardReadyChanged();
        }
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

    const QDBusReply<QString> reply = iface.call(QStringLiteral("GetRecentLogsJson"), 100U);
    if (!reply.isValid()) {
        return;
    }

    const QJsonDocument document = QJsonDocument::fromJson(reply.value().toUtf8());
    if (!document.isArray()) {
        return;
    }

    const QVariantList entries = document.array().toVariantList();
    QStringList lines;
    lines.reserve(entries.size());
    for (const QVariant &entryVariant : entries) {
        const QVariantMap entry = entryVariant.toMap();
        const QString timestamp = formatTimestamp(entry.value(QStringLiteral("timestamp_unix")).toLongLong());
        const QString source = entry.value(QStringLiteral("source")).toString();
        const QString level = entry.value(QStringLiteral("level")).toString().toUpper();
        const QString message = entry.value(QStringLiteral("message")).toString();
        lines << QStringLiteral("[%1] [%2] [%3] %4").arg(timestamp,
                                                         level.isEmpty() ? QStringLiteral("INFO") : level,
                                                         source.isEmpty() ? QStringLiteral("daemon") : source,
                                                         message);
    }

    if (entries != m_recentLogEntries || lines != m_recentLogs) {
        m_recentLogEntries = entries;
        m_recentLogs = lines;
        emit recentLogsChanged();
    }
}

void ShellBackend::applyStatusJson(const QString &jsonPayload)
{
    const QString previousAppState = appState();
    const bool wasDashboardReady = dashboardReady();
    const QJsonDocument document = QJsonDocument::fromJson(jsonPayload.toUtf8());
    if (!document.isObject()) {
        updateStatusMessage(tr("Daemon returned malformed status JSON."));
        return;
    }

    if (!m_daemonReachable) {
        m_daemonReachable = true;
        emit daemonReachableChanged();
        emit mountStateChanged();
        emit syncStateChanged();
    }

    const QJsonObject object = document.object();
    const bool remoteConfigured = object.value(QStringLiteral("remote_configured")).toBool();
    const bool needsRemoteRepair = object.value(QStringLiteral("needs_remote_repair")).toBool();
    const QString mountPath = object.value(QStringLiteral("root_path")).toString();
    const QString connectionState = object.value(QStringLiteral("connection_state")).toString();
    const QString mountState = object.value(QStringLiteral("filesystem_state")).toString();
    const QString syncState = object.value(QStringLiteral("sync_state")).toString();
    const QString lastError = object.value(QStringLiteral("last_error")).toString();
    const QString lastSyncError = object.value(QStringLiteral("last_sync_error")).toString();
    const QString lastLogLine = object.value(QStringLiteral("last_log_line")).toString();
    const QString rcloneVersion = object.value(QStringLiteral("rclone_version")).toString();
    const bool customClientIdConfigured = object.value(QStringLiteral("custom_client_id_configured")).toBool();
    const qint64 cacheBytes = object.value(QStringLiteral("backing_usage_bytes")).toInteger();
    const QString backingDirName = object.value(QStringLiteral("backing_dir_name")).toString();
    const int pinnedFileCount = object.value(QStringLiteral("pinned_file_count")).toInt();
    const int pendingDownloads = object.value(QStringLiteral("pending_downloads")).toInt();
    const int pendingUploads = object.value(QStringLiteral("pending_uploads")).toInt();
    const int conflictCount = object.value(QStringLiteral("conflict_count")).toInt();
    const qint64 lastSyncAt = object.value(QStringLiteral("last_sync_at")).toInteger();
    const int queueDepth = pendingDownloads + pendingUploads;
    const int activeTransferCount = pendingDownloads + pendingUploads;
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

    if (needsRemoteRepair != m_needsRemoteRepair) {
        m_needsRemoteRepair = needsRemoteRepair;
        emit needsRemoteRepairChanged();
        emit mountStateChanged();
    }

    if (customClientIdConfigured != m_customClientIdConfigured) {
        m_customClientIdConfigured = customClientIdConfigured;
        emit customClientIdConfiguredChanged();
    }

    bool mountChanged = false;
    if (!connectionState.isEmpty() && connectionState != m_connectionState) {
        m_connectionState = connectionState;
        emit connectionStateChanged();
        mountChanged = true;
    }

    if (!mountState.isEmpty() && mountState != m_mountState) {
        m_mountState = mountState;
        mountChanged = true;
    }
    if (mountChanged) {
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
    if (pendingDownloads != m_pendingDownloads) {
        m_pendingDownloads = pendingDownloads;
        syncChanged = true;
    }
    if (pendingUploads != m_pendingUploads) {
        m_pendingUploads = pendingUploads;
        syncChanged = true;
    }
    if (conflictCount != m_conflictCount) {
        m_conflictCount = conflictCount;
        syncChanged = true;
    }
    if (syncChanged) {
        emit syncStateChanged();
    }

    const QString cacheLabel = tr("%1 in %2").arg(formatBytes(cacheBytes),
                                                  backingDirName.isEmpty() ? m_backingDirName
                                                                           : backingDirName);
    if (cacheLabel != m_cacheUsageLabel) {
        m_cacheUsageLabel = cacheLabel;
        emit cacheUsageLabelChanged();
    }

    if (!backingDirName.isEmpty() && backingDirName != m_backingDirName) {
        m_backingDirName = backingDirName;
        emit backingDirNameChanged();
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
    if (previousAppState != appState()) {
        emit appStateChanged();
    }

    if (m_needsRemoteRepair) {
        updateStatusMessage(tr("The app-owned OneDrive profile came from an older release. Use Repair Remote to rebuild only the rclone profile and keep hydrated bytes plus path state on this device."));
        updateTray();
        return;
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
        updateStatusMessage(tr("Choose a OneDrive root folder, then start the browser sign-in."));
        updateTray();
        return;
    }

    if (m_connectionState == QStringLiteral("Connecting")) {
        updateStatusMessage(tr("Browser sign-in is in progress. Finish the Microsoft login flow."));
        updateTray();
        return;
    }

    if (m_connectionState == QStringLiteral("Error")) {
        updateStatusMessage(tr("Connection needs attention. Review the recent logs and retry the sign-in flow if needed."));
        updateTray();
        return;
    }

    if (m_mountState == QStringLiteral("Starting")) {
        updateStatusMessage(tr("Starting the local OneDrive filesystem at %1.").arg(m_effectiveMountPath));
        updateTray();
        return;
    }

    if (m_syncState == QStringLiteral("Scanning")) {
        updateStatusMessage(tr("Refreshing remote state and updating Dolphin overlays."));
        updateTray();
        return;
    }

    if (m_syncState == QStringLiteral("Paused")) {
        updateStatusMessage(tr("Background sync is paused. On-demand opens still work, and local writes stay queued until you resume sync."));
        updateTray();
        return;
    }

    if (m_syncState == QStringLiteral("Syncing")) {
        updateStatusMessage(tr("Applying residency changes and syncing pending transfers."));
        updateTray();
        return;
    }

    if (m_conflictCount > 0) {
        updateStatusMessage(tr("%1 item(s) need manual conflict recovery. Retry the transfer after reviewing the file.").arg(m_conflictCount));
        updateTray();
        return;
    }

    if (m_mountState == QStringLiteral("Running")) {
        updateStatusMessage(tr("The OneDrive root is active at %1.").arg(m_effectiveMountPath));
        updateTray();
        return;
    }

    if (m_mountState == QStringLiteral("Stopped")) {
        updateStatusMessage(tr("OneDrive is connected, but the local filesystem is currently stopped."));
        updateTray();
        return;
    }

    if (m_mountState == QStringLiteral("Degraded")) {
        updateStatusMessage(tr("The filesystem is running in a degraded state. Review recent logs and retry failed transfers."));
        updateTray();
        return;
    }

    updateStatusMessage(tr("Review recent logs and retry the filesystem if needed."));
    updateTray();
}

void ShellBackend::connectDaemonSignals()
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    bus.connect(QString::fromLatin1(kService),
                QString::fromLatin1(kPath),
                QString::fromLatin1(kInterface),
                QStringLiteral("ConnectionStateChanged"),
                this,
                SLOT(onDaemonActivity()));
    bus.connect(QString::fromLatin1(kService),
                QString::fromLatin1(kPath),
                QString::fromLatin1(kInterface),
                QStringLiteral("FilesystemStateChanged"),
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
                SLOT(onPathStatesChanged()));
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
    m_mountAction = m_trayMenu->addAction(tr("Start Filesystem"));
    m_unmountAction = m_trayMenu->addAction(tr("Stop Filesystem"));
    m_rescanAction = m_trayMenu->addAction(tr("Rescan"));
    m_pauseSyncAction = m_trayMenu->addAction(tr("Pause Sync"));
    m_resumeSyncAction = m_trayMenu->addAction(tr("Resume Sync"));
    m_trayMenu->addSeparator();
    m_quitAction = m_trayMenu->addAction(tr("Quit"));

    connect(m_showWindowAction, &QAction::triggered, this, [this]() {
        activateMainWindow();
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
            activateMainWindow();
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
    const QDBusReply<void> pathReply = iface.call(QStringLiteral("SetRootPath"), m_mountPath);
    if (!pathReply.isValid()) {
        updateStatusMessage(tr("Root folder update failed: %1").arg(pathReply.error().message()));
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
    m_rescanAction->setEnabled(m_daemonReachable && m_remoteConfigured);
    m_pauseSyncAction->setEnabled(canPauseSync());
    m_resumeSyncAction->setEnabled(canResumeSync());

    auto trayStatus = KStatusNotifierItem::Active;
    QString overlayIcon;
    if (!m_daemonReachable) {
        trayStatus = KStatusNotifierItem::NeedsAttention;
        overlayIcon = QStringLiteral("network-disconnect");
    } else if (m_mountState == QStringLiteral("Error")
        || m_connectionState == QStringLiteral("Error")
        || m_syncState == QStringLiteral("Error")
        || m_conflictCount > 0) {
        trayStatus = KStatusNotifierItem::NeedsAttention;
        overlayIcon = QStringLiteral("emblem-important");
    } else if (m_connectionState == QStringLiteral("Connecting")
               || m_mountState == QStringLiteral("Starting")
               || m_syncState == QStringLiteral("Scanning")
               || m_syncState == QStringLiteral("Syncing")) {
        overlayIcon = QStringLiteral("emblem-synchronizing");
    } else if (m_syncState == QStringLiteral("Paused")) {
        overlayIcon = QStringLiteral("media-playback-pause");
    } else if (!m_remoteConfigured) {
        trayStatus = KStatusNotifierItem::Passive;
    } else if (m_mountState == QStringLiteral("Running")) {
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
        updateStatusMessage(tr("%1 failed: %2").arg(actionLabelForMethod(method), reply.error().message()));
        return false;
    }

    updateStatusMessage(
        tr("%1 applied to %2 item(s).").arg(actionLabelForMethod(method), QString::number(reply.value())));
    return true;
}

bool ShellBackend::invokePathsAction(const QString &method,
                                     const QStringList &paths,
                                     const QString &emptyPathMessage)
{
    QStringList normalizedPaths;
    normalizedPaths.reserve(paths.size());
    for (const QString &path : paths) {
        const QString normalizedPath = normalizeMountPath(path);
        if (!normalizedPath.isEmpty()) {
            normalizedPaths << normalizedPath;
        }
    }

    normalizedPaths.removeDuplicates();
    if (normalizedPaths.isEmpty()) {
        updateStatusMessage(emptyPathMessage);
        return false;
    }

    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(tr("Daemon not reachable on D-Bus."));
        return false;
    }

    const QDBusReply<uint> reply = iface.call(method, normalizedPaths);
    if (!reply.isValid()) {
        updateStatusMessage(tr("%1 failed: %2").arg(actionLabelForMethod(method), reply.error().message()));
        return false;
    }

    updateStatusMessage(
        tr("%1 applied to %2 item(s).").arg(actionLabelForMethod(method), QString::number(reply.value())));
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

void ShellBackend::onPathStatesChanged()
{
    refreshStatus();
    emit pathStatesChanged();
}

void ShellBackend::onLogsUpdated()
{
    refreshLogs();
}

void ShellBackend::onErrorRaised(const QString &message)
{
    const bool shouldNotify = m_tray != nullptr
                              && (m_mainWindow == nullptr
                                  || !m_mainWindow->isVisible()
                                  || !m_mainWindow->isActive());
    if (shouldNotify) {
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
