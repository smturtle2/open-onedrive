#include "appcontroller.h"

AppController::AppController(QObject *parent)
    : QObject(parent),
      m_mountPath(QStringLiteral("%1/OneDrive").arg(qEnvironmentVariable("HOME"))) {}

bool AppController::configured() const {
    return !m_clientId.isEmpty();
}

bool AppController::paused() const {
    return m_paused;
}

QString AppController::clientId() const {
    return m_clientId;
}

QString AppController::mountPath() const {
    return m_mountPath;
}

QString AppController::syncState() const {
    return m_paused ? QStringLiteral("Paused") : QStringLiteral("Idle");
}

QString AppController::cacheUsage() const {
    return QStringLiteral("0 B / 25 GiB");
}

void AppController::saveSetup(const QString &clientId, const QString &mountPath) {
    if (clientId.trimmed().isEmpty()) {
        emit toastRequested(QStringLiteral("Client ID is required."));
        return;
    }
    if (mountPath.trimmed().isEmpty()) {
        emit toastRequested(QStringLiteral("Mount path is required."));
        return;
    }

    m_clientId = clientId.trimmed();
    m_mountPath = mountPath.trimmed();
    emit configuredChanged();
    emit mountPathChanged();
    emit statusChanged();
    emit toastRequested(QStringLiteral("Setup saved. D-Bus integration is the next step."));
}

void AppController::toggleSync() {
    m_paused = !m_paused;
    emit pausedChanged();
    emit statusChanged();
}

void AppController::refreshStatus() {
    emit toastRequested(QStringLiteral("Status refresh is currently a local placeholder."));
    emit statusChanged();
}

