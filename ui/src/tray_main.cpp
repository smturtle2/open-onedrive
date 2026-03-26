#include "ShellBackend.h"

#include <KDBusService>

#include <QApplication>

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    app.setOrganizationName(QStringLiteral("smturtle2"));
    app.setOrganizationDomain(QStringLiteral("github.io"));
    app.setApplicationName(QStringLiteral("open-onedrive-tray"));
    app.setQuitOnLastWindowClosed(false);

    KDBusService service(KDBusService::Unique);
    ShellBackend backend(true);
    QObject::connect(&service,
                     &KDBusService::activateRequested,
                     &backend,
                     [&backend](const QStringList &, const QString &) { backend.activateMainWindow(); });

    return app.exec();
}
