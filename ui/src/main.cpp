#include "ShellBackend.h"

#include <QApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QWindow>

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    app.setOrganizationName(QStringLiteral("smturtle2"));
    app.setOrganizationDomain(QStringLiteral("github.io"));
    app.setApplicationName(QStringLiteral("open-onedrive"));

    QQmlApplicationEngine engine;
    ShellBackend backend;
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

    return app.exec();
}
