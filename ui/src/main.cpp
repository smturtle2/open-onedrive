#include "ShellBackend.h"

#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>

int main(int argc, char *argv[])
{
    QGuiApplication app(argc, argv);
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

    return app.exec();
}

