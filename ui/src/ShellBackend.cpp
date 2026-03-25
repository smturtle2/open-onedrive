#include "ShellBackend.h"

#include <QDBusInterface>
#include <QDBusReply>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTimer>
#include <QUrl>

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
    m_refreshTimer = new QTimer(this);
    m_refreshTimer->setInterval(3000);
    connect(m_refreshTimer, &QTimer::timeout, this, &ShellBackend::refreshStatus);
    connect(m_refreshTimer, &QTimer::timeout, this, &ShellBackend::refreshLogs);
    m_refreshTimer->start();
    QTimer::singleShot(0, this, &ShellBackend::refreshStatus);
    QTimer::singleShot(0, this, &ShellBackend::refreshLogs);
}

bool ShellBackend::remoteConfigured() const
{
    return m_remoteConfigured;
}

bool ShellBackend::dashboardReady() const
{
    return m_remoteConfigured && m_mountState != QStringLiteral("Error");
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

QString ShellBackend::statusMessage() const
{
    return m_statusMessage;
}

QString ShellBackend::cacheUsageLabel() const
{
    return m_cacheUsageLabel;
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

void ShellBackend::beginConnect()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(QStringLiteral("Daemon not reachable on D-Bus. Start openonedrived first."));
        return;
    }

    if (!syncMountPathIfNeeded(iface, QStringLiteral("Choose a mount path before connecting."))) {
        return;
    }

    const QDBusReply<void> connectReply = iface.call(QStringLiteral("BeginConnect"));
    if (!connectReply.isValid()) {
        updateStatusMessage(QStringLiteral("Connect failed: %1").arg(connectReply.error().message()));
        return;
    }

    updateStatusMessage(QStringLiteral("Started the rclone browser sign-in flow. Finish the Microsoft login in your browser."));
    refreshStatus();
    refreshLogs();
}

void ShellBackend::disconnectRemote()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(QStringLiteral("Daemon not reachable on D-Bus."));
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("Disconnect"));
    if (!reply.isValid()) {
        updateStatusMessage(QStringLiteral("Disconnect failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
    refreshLogs();
}

void ShellBackend::mountRemote()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(QStringLiteral("Daemon not reachable on D-Bus."));
        return;
    }

    if (!syncMountPathIfNeeded(iface, QStringLiteral("Choose a mount path before mounting."))) {
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("Mount"));
    if (!reply.isValid()) {
        updateStatusMessage(QStringLiteral("Mount failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::unmountRemote()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(QStringLiteral("Daemon not reachable on D-Bus."));
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("Unmount"));
    if (!reply.isValid()) {
        updateStatusMessage(QStringLiteral("Unmount failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::retryMount()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(QStringLiteral("Daemon not reachable on D-Bus."));
        return;
    }

    if (!syncMountPathIfNeeded(iface, QStringLiteral("Choose a mount path before retrying."))) {
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("RetryMount"));
    if (!reply.isValid()) {
        updateStatusMessage(QStringLiteral("Retry failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::openMountLocation()
{
    if (m_effectiveMountPath.isEmpty()) {
        updateStatusMessage(QStringLiteral("Choose a mount path first."));
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

void ShellBackend::refreshStatus()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(QStringLiteral("Daemon not reachable on D-Bus. UI is in local fallback mode."));
        return;
    }

    const QDBusReply<QString> reply = iface.call(QStringLiteral("GetStatusJson"));
    if (!reply.isValid()) {
        updateStatusMessage(QStringLiteral("Status refresh failed: %1").arg(reply.error().message()));
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
        updateStatusMessage(QStringLiteral("Daemon returned malformed status JSON."));
        return;
    }

    const QJsonObject object = document.object();
    const bool remoteConfigured = object.value(QStringLiteral("remote_configured")).toBool();
    const QString mountPath = object.value(QStringLiteral("mount_path")).toString();
    const QString mountState = object.value(QStringLiteral("mount_state")).toString();
    const QString lastError = object.value(QStringLiteral("last_error")).toString();
    const QString lastLogLine = object.value(QStringLiteral("last_log_line")).toString();
    const QString rcloneVersion = object.value(QStringLiteral("rclone_version")).toString();
    const bool customClientIdConfigured = object.value(QStringLiteral("custom_client_id_configured")).toBool();
    const qint64 cacheBytes = object.value(QStringLiteral("cache_usage_bytes")).toInteger();

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

    const QString cacheLabel = QStringLiteral("%1 B cached").arg(cacheBytes);
    if (cacheLabel != m_cacheUsageLabel) {
        m_cacheUsageLabel = cacheLabel;
        emit cacheUsageLabelChanged();
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
        return;
    }

    if (!m_remoteConfigured) {
        updateStatusMessage(QStringLiteral("Choose a mount folder, then start the OneDrive browser sign-in."));
        return;
    }

    if (m_mountState == QStringLiteral("Connecting")) {
        updateStatusMessage(QStringLiteral("Browser sign-in is in progress. Finish the Microsoft login flow."));
        return;
    }

    if (m_mountState == QStringLiteral("Mounted")) {
        updateStatusMessage(QStringLiteral("rclone mount is active at %1.").arg(m_effectiveMountPath));
        return;
    }

    if (m_mountState == QStringLiteral("Mounting")) {
        updateStatusMessage(QStringLiteral("Starting rclone mount."));
        return;
    }

    if (m_mountState == QStringLiteral("Unmounted")) {
        updateStatusMessage(QStringLiteral("OneDrive is configured but currently unmounted."));
        return;
    }

    updateStatusMessage(QStringLiteral("Review recent logs and reconnect if needed."));
}

void ShellBackend::updateStatusMessage(const QString &message)
{
    if (m_statusMessage == message) {
        return;
    }

    m_statusMessage = message;
    emit statusMessageChanged();
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
        updateStatusMessage(QStringLiteral("Mount path update failed: %1").arg(pathReply.error().message()));
        return false;
    }

    m_effectiveMountPath = m_mountPath;
    emit effectiveMountPathChanged();
    if (pendingBefore != mountPathPending()) {
        emit mountPathPendingChanged();
    }
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
