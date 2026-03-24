#include "ActionPlugin.hpp"

#include <KFileItemListProperties>
#include <KPluginFactory>
#include <QAction>
#include <QDBusInterface>
#include <QDBusReply>
#include <QDesktopServices>
#include <QWidget>

namespace {
constexpr auto kService = "io.github.smturtle2.OpenOneDrive1";
constexpr auto kPath = "/io/github/smturtle2/OpenOneDrive1";
constexpr auto kInterface = "io.github.smturtle2.OpenOneDrive1";

QStringList localPaths(const KFileItemListProperties &items)
{
    QStringList result;
    for (const QUrl &url : items.urlList()) {
        if (url.isLocalFile()) {
            result << url.toLocalFile();
        }
    }
    return result;
}
}

K_PLUGIN_CLASS_WITH_JSON(OpenOneDriveActionPlugin, "../metadata.json")

QList<QAction *> OpenOneDriveActionPlugin::actions(const KFileItemListProperties &fileItemInfos, QWidget *parentWidget)
{
    if (fileItemInfos.urlList().isEmpty()) {
        return {};
    }

    QObject *actionParent = parentWidget != nullptr ? static_cast<QObject *>(parentWidget) : this;
    auto *pinAction = new QAction(QStringLiteral("Always keep on this device"), actionParent);
    QObject::connect(pinAction, &QAction::triggered, this, [items = fileItemInfos] {
        QDBusInterface iface(QString::fromLatin1(kService),
                             QString::fromLatin1(kPath),
                             QString::fromLatin1(kInterface),
                             QDBusConnection::sessionBus());
        iface.call(QStringLiteral("Pin"), localPaths(items));
    });

    auto *evictAction = new QAction(QStringLiteral("Free up space"), actionParent);
    QObject::connect(evictAction, &QAction::triggered, this, [items = fileItemInfos] {
        QDBusInterface iface(QString::fromLatin1(kService),
                             QString::fromLatin1(kPath),
                             QString::fromLatin1(kInterface),
                             QDBusConnection::sessionBus());
        iface.call(QStringLiteral("Evict"), localPaths(items));
    });

    auto *browserAction = new QAction(QStringLiteral("Open in OneDrive"), actionParent);
    QObject::connect(browserAction, &QAction::triggered, this, [items = fileItemInfos] {
        const QStringList paths = localPaths(items);
        if (paths.isEmpty()) {
            return;
        }

        QDBusInterface iface(QString::fromLatin1(kService),
                             QString::fromLatin1(kPath),
                             QString::fromLatin1(kInterface),
                             QDBusConnection::sessionBus());
        const QDBusReply<QString> reply = iface.call(QStringLiteral("OpenInBrowser"), paths.constFirst());
        if (reply.isValid()) {
            QDesktopServices::openUrl(QUrl(reply.value()));
        }
    });

    QList<QAction *> actions;
    actions << pinAction << evictAction << browserAction;
    return actions;
}

#include "ActionPlugin.moc"
