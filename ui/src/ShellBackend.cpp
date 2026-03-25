#include "ShellBackend.h"

#include <QDBusInterface>
#include <QDBusReply>
#include <QDesktopServices>
#include <QDir>
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
}

ShellBackend::ShellBackend(QObject *parent)
    : QObject(parent)
{
    m_mountPath = QStringLiteral("%1/OneDrive").arg(qEnvironmentVariable("HOME"));
    m_refreshTimer = new QTimer(this);
    m_refreshTimer->setInterval(3000);
    connect(m_refreshTimer, &QTimer::timeout, this, &ShellBackend::refreshStatus);
    m_refreshTimer->start();
    QTimer::singleShot(0, this, &ShellBackend::refreshStatus);
}

bool ShellBackend::configured() const
{
    return m_configured;
}

QString ShellBackend::clientId() const
{
    return m_clientId;
}

QString ShellBackend::mountPath() const
{
    return m_mountPath;
}

QString ShellBackend::syncState() const
{
    return m_syncState;
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

QString ShellBackend::indexedItemsLabel() const
{
    return m_indexedItemsLabel;
}

void ShellBackend::setClientId(const QString &clientId)
{
    if (m_clientId == clientId) {
        return;
    }

    m_clientId = clientId;
    emit clientIdChanged();
    updateConfigured();
}

void ShellBackend::setMountPath(const QString &mountPath)
{
    if (m_mountPath == mountPath) {
        return;
    }

    m_mountPath = mountPath;
    emit mountPathChanged();
    updateConfigured();
}

void ShellBackend::completeSetup()
{
    if (m_clientId.trimmed().isEmpty() || m_mountPath.trimmed().isEmpty()) {
        updateStatusMessage(QStringLiteral("Client ID and mount path are both required."));
        return;
    }

    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(QStringLiteral("Daemon not reachable on D-Bus. Start openonedrived first."));
        return;
    }

    const QDBusReply<void> mountReply = iface.call(QStringLiteral("SetMountPath"), m_mountPath.trimmed());
    if (!mountReply.isValid()) {
        updateStatusMessage(QStringLiteral("Mount path update failed: %1").arg(mountReply.error().message()));
        return;
    }

    const QDBusReply<QString> loginReply = iface.call(QStringLiteral("Login"), m_clientId.trimmed());
    if (!loginReply.isValid()) {
        updateStatusMessage(QStringLiteral("Login setup failed: %1").arg(loginReply.error().message()));
        return;
    }

    m_configured = true;
    emit configuredChanged();
    QDesktopServices::openUrl(QUrl(loginReply.value()));
    updateStatusMessage(QStringLiteral("Opened browser for Microsoft sign-in."));
    refreshStatus();
}

void ShellBackend::pauseSync()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(QStringLiteral("Daemon not reachable on D-Bus."));
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("PauseSync"));
    if (!reply.isValid()) {
        updateStatusMessage(QStringLiteral("Pause failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::resumeSync()
{
    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        updateStatusMessage(QStringLiteral("Daemon not reachable on D-Bus."));
        return;
    }

    const QDBusReply<void> reply = iface.call(QStringLiteral("ResumeSync"));
    if (!reply.isValid()) {
        updateStatusMessage(QStringLiteral("Resume failed: %1").arg(reply.error().message()));
        return;
    }

    refreshStatus();
}

void ShellBackend::openMountLocation()
{
    if (m_mountPath.trimmed().isEmpty()) {
        updateStatusMessage(QStringLiteral("Choose a mount path first."));
        return;
    }

    QDesktopServices::openUrl(QUrl::fromLocalFile(QDir::cleanPath(m_mountPath)));
}

void ShellBackend::freeUpSpace()
{
    updateStatusMessage(QStringLiteral("Per-file evict lives in the Dolphin plugin scaffold."));
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

void ShellBackend::applyStatusJson(const QString &jsonPayload)
{
    const QJsonDocument document = QJsonDocument::fromJson(jsonPayload.toUtf8());
    if (!document.isObject()) {
        updateStatusMessage(QStringLiteral("Daemon returned malformed status JSON."));
        return;
    }

    const QJsonObject object = document.object();
    const QString mountPath = object.value(QStringLiteral("mount_path")).toString();
    const QString syncState = object.value(QStringLiteral("sync_state")).toString();
    const QString mountState = object.value(QStringLiteral("mount_state")).toString();
    const QString lastError = object.value(QStringLiteral("last_error")).toString();
    const qint64 cacheBytes = object.value(QStringLiteral("cache_usage_bytes")).toInteger();
    const qint64 itemsIndexed = object.value(QStringLiteral("items_indexed")).toInteger();

    if (!mountPath.isEmpty() && mountPath != m_mountPath) {
        m_mountPath = mountPath;
        emit mountPathChanged();
    }

    const bool configured = object.value(QStringLiteral("client_id_configured")).toBool();
    if (configured != m_configured) {
        m_configured = configured;
        emit configuredChanged();
    }

    if (!syncState.isEmpty() && syncState != m_syncState) {
        m_syncState = syncState;
        emit syncStateChanged();
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

    const QString indexedLabel = QStringLiteral("%1 items indexed").arg(itemsIndexed);
    if (indexedLabel != m_indexedItemsLabel) {
        m_indexedItemsLabel = indexedLabel;
        emit indexedItemsLabelChanged();
    }

    if (!lastError.isEmpty()) {
        updateStatusMessage(lastError);
        return;
    }

    updateStatusMessage(QStringLiteral("Daemon connected. %1, %2.").arg(m_syncState, m_indexedItemsLabel));
}

void ShellBackend::updateStatusMessage(const QString &message)
{
    if (m_statusMessage == message) {
        return;
    }

    m_statusMessage = message;
    emit statusMessageChanged();
}

void ShellBackend::updateConfigured()
{
    const bool configured = !m_clientId.trimmed().isEmpty() && !m_mountPath.trimmed().isEmpty();
    if (m_configured == configured) {
        return;
    }

    m_configured = configured;
    emit configuredChanged();
}
