#pragma once

#include <KAbstractFileItemActionPlugin>

class OpenOneDriveActionPlugin : public KAbstractFileItemActionPlugin
{
    Q_OBJECT

public:
    using KAbstractFileItemActionPlugin::KAbstractFileItemActionPlugin;

    QList<QAction *> actions(const KFileItemListProperties &fileItemInfos, QWidget *parentWidget) override;
};

