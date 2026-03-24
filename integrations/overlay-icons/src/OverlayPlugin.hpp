#pragma once

#include <KOverlayIconPlugin>

class OpenOneDriveOverlayPlugin : public KOverlayIconPlugin
{
    Q_OBJECT

public:
    using KOverlayIconPlugin::KOverlayIconPlugin;

    QStringList getOverlays(const QUrl &item) override;
};

