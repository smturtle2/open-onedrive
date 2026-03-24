#pragma once

#include <QObject>
#include <QString>

class AppController final : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool configured READ configured NOTIFY configuredChanged)
    Q_PROPERTY(bool paused READ paused NOTIFY pausedChanged)
    Q_PROPERTY(QString clientId READ clientId NOTIFY configuredChanged)
    Q_PROPERTY(QString mountPath READ mountPath NOTIFY mountPathChanged)
    Q_PROPERTY(QString syncState READ syncState NOTIFY statusChanged)
    Q_PROPERTY(QString cacheUsage READ cacheUsage NOTIFY statusChanged)

public:
    explicit AppController(QObject *parent = nullptr);

    [[nodiscard]] bool configured() const;
    [[nodiscard]] bool paused() const;
    [[nodiscard]] QString clientId() const;
    [[nodiscard]] QString mountPath() const;
    [[nodiscard]] QString syncState() const;
    [[nodiscard]] QString cacheUsage() const;

    Q_INVOKABLE void saveSetup(const QString &clientId, const QString &mountPath);
    Q_INVOKABLE void toggleSync();
    Q_INVOKABLE void refreshStatus();

signals:
    void configuredChanged();
    void mountPathChanged();
    void pausedChanged();
    void statusChanged();
    void toastRequested(const QString &message);

private:
    QString m_clientId;
    QString m_mountPath;
    bool m_paused = false;
};

