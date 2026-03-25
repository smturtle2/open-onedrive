#include "ActionPlugin.hpp"

#include <KFileItem>
#include <KFileItemListProperties>
#include <KPluginFactory>
#include <QAction>
#include <QDBusInterface>
#include <QDBusReply>
#include <QDesktopServices>
#include <QDir>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStringList>
#include <QWidget>

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

QJsonObject statusObject()
{
    QDBusInterface iface = daemonInterface();
    const QDBusReply<QString> reply = iface.call(QStringLiteral("GetStatusJson"));
    if (!reply.isValid()) {
        return {};
    }

    const QJsonDocument document = QJsonDocument::fromJson(reply.value().toUtf8());
    if (!document.isObject()) {
        return {};
    }

    return document.object();
}

QJsonArray pathStates(const QStringList &paths)
{
    if (paths.isEmpty()) {
        return {};
    }

    QDBusInterface iface = daemonInterface();
    const QDBusReply<QString> reply = iface.call(QStringLiteral("GetPathStatesJson"), paths);
    if (!reply.isValid()) {
        return {};
    }

    const QJsonDocument document = QJsonDocument::fromJson(reply.value().toUtf8());
    if (!document.isArray()) {
        return {};
    }

    return document.array();
}

QString rootFolder(const QJsonObject &status)
{
    return QDir::cleanPath(status.value(QStringLiteral("root_path")).toString());
}

QStringList selectedLocalPaths(const KFileItemListProperties &fileItemInfos)
{
    QStringList paths;
    for (const auto &item : fileItemInfos.items()) {
        const QString localPath = item.localPath();
        if (!localPath.isEmpty()) {
            paths << QDir::cleanPath(localPath);
        }
    }
    return paths;
}

bool isWithinVisibleRoot(const QString &path, const QString &rootPath, const QString &backingDirName)
{
    if (path.isEmpty() || rootPath.isEmpty()) {
        return false;
    }

    if (path != rootPath && !path.startsWith(rootPath + QLatin1Char('/'))) {
        return false;
    }

    if (backingDirName.isEmpty()) {
        return true;
    }

    const QString hiddenRoot = QDir::cleanPath(rootPath + QLatin1Char('/') + backingDirName);
    return path != hiddenRoot && !path.startsWith(hiddenRoot + QLatin1Char('/'));
}

bool invokePathAction(const QString &method, const QStringList &paths)
{
    if (paths.isEmpty()) {
        return false;
    }

    QDBusInterface iface = daemonInterface();
    if (!iface.isValid()) {
        return false;
    }

    const QDBusReply<uint> reply = iface.call(method, paths);
    return reply.isValid();
}
}

K_PLUGIN_CLASS_WITH_JSON(OpenOneDriveActionPlugin, "../metadata.json")

QList<QAction *> OpenOneDriveActionPlugin::actions(const KFileItemListProperties &fileItemInfos, QWidget *parentWidget)
{
    const QJsonObject status = statusObject();
    if (!status.value(QStringLiteral("remote_configured")).toBool()) {
        return {};
    }

    const QString rootPath = rootFolder(status);
    const QString backingDirName = status.value(QStringLiteral("backing_dir_name")).toString();
    const QStringList selectedPaths = selectedLocalPaths(fileItemInfos);
    if (rootPath.isEmpty() || selectedPaths.isEmpty()) {
        return {};
    }

    for (const QString &path : selectedPaths) {
        if (!isWithinVisibleRoot(path, rootPath, backingDirName)) {
            return {};
        }
    }

    bool showKeepLocal = true;
    bool showOnlineOnly = true;
    bool showRetryTransfer = false;
    const QJsonArray states = pathStates(selectedPaths);
    if (!states.isEmpty()) {
        bool anyOnlineOnly = false;
        bool anyLocal = false;
        for (const QJsonValue &value : states) {
            const QString state = value.toObject().value(QStringLiteral("state")).toString();
            anyOnlineOnly |= state == QStringLiteral("OnlineOnly");
            anyLocal |= state == QStringLiteral("PinnedLocal") || state == QStringLiteral("AvailableLocal");
            showRetryTransfer |= state == QStringLiteral("Conflict") || state == QStringLiteral("Error");
        }
        showKeepLocal = anyOnlineOnly;
        showOnlineOnly = anyLocal;
    }

    QObject *actionParent = parentWidget != nullptr ? static_cast<QObject *>(parentWidget) : this;
    QList<QAction *> actions;
    if (showKeepLocal) {
        auto *keepLocalAction = new QAction(QStringLiteral("Keep on this device"), actionParent);
        QObject::connect(keepLocalAction, &QAction::triggered, this, [selectedPaths] {
            invokePathAction(QStringLiteral("KeepLocal"), selectedPaths);
        });
        actions << keepLocalAction;
    }

    if (showOnlineOnly) {
        auto *onlineOnlyAction = new QAction(QStringLiteral("Make online-only"), actionParent);
        QObject::connect(onlineOnlyAction, &QAction::triggered, this, [selectedPaths] {
            invokePathAction(QStringLiteral("MakeOnlineOnly"), selectedPaths);
        });
        actions << onlineOnlyAction;
    }

    if (showRetryTransfer) {
        auto *retryTransferAction = new QAction(QStringLiteral("Retry transfer"), actionParent);
        QObject::connect(retryTransferAction, &QAction::triggered, this, [selectedPaths] {
            invokePathAction(QStringLiteral("RetryTransfer"), selectedPaths);
        });
        actions << retryTransferAction;
    }

    auto *openRootAction = new QAction(QStringLiteral("Open OneDrive Root"), actionParent);
    QObject::connect(openRootAction, &QAction::triggered, this, [rootPath] {
        QDesktopServices::openUrl(QUrl::fromLocalFile(rootPath));
    });
    actions << openRootAction;
    return actions;
}

#include "ActionPlugin.moc"
