#include "ShellBackend.h"

#include <KDBusService>

#include <QApplication>
#include <QDBusConnection>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QWindow>

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    app.setOrganizationName(QStringLiteral("smturtle2"));
    app.setOrganizationDomain(QStringLiteral("github.io"));
    app.setApplicationName(QStringLiteral("open-onedrive"));

    KDBusService service(KDBusService::Unique);
    QQmlApplicationEngine engine;
    ShellBackend backend(false);
    engine.rootContext()->setContextProperty(QStringLiteral("shellBackend"), &backend);

    const QUrl mainUrl(QStringLiteral("qrc:/qml/Main.qml"));
    QObject::connect(
        &engine,
        &QQmlApplicationEngine::objectCreationFailed,
        &app,
        []() { QCoreApplication::exit(1); },
        Qt::QueuedConnection);
    engine.load(mainUrl);
    if (!engine.rootObjects().isEmpty()) {
        backend.setMainWindow(qobject_cast<QWindow *>(engine.rootObjects().constFirst()));
    }
    QDBusConnection::sessionBus().registerService(QStringLiteral("io.github.smturtle2.OpenOneDriveUi1"));
    QDBusConnection::sessionBus().registerObject(QStringLiteral("/io/github/smturtle2/OpenOneDriveUi1"),
                                                 &backend,
                                                 QDBusConnection::ExportAllInvokables);
    QObject::connect(&service,
                     &KDBusService::activateRequested,
                     &backend,
                     [&backend](const QStringList &, const QString &) { backend.ActivateMainWindow(); });

    return app.exec();
}
