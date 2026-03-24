use crate::app::OpenOneDriveApp;
use openonedrive_ipc_types::{ItemSnapshot, MountState, StatusSnapshot};
use std::sync::Arc;
use zbus::{SignalContext, interface};

pub const DBUS_SERVICE: &str = "io.github.smturtle2.OpenOneDrive1";
pub const DBUS_PATH: &str = "/io/github/smturtle2/OpenOneDrive1";

pub struct OpenOneDriveBus {
    app: Arc<OpenOneDriveApp>,
}

impl OpenOneDriveBus {
    pub fn new(app: Arc<OpenOneDriveApp>) -> Self {
        Self { app }
    }
}

fn map_error(error: anyhow::Error) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(error.to_string())
}

#[interface(name = "io.github.smturtle2.OpenOneDrive1")]
impl OpenOneDriveBus {
    async fn login(&self, client_id: &str) -> zbus::fdo::Result<String> {
        self.app.login(client_id).await.map_err(map_error)
    }

    async fn logout(&self) -> zbus::fdo::Result<()> {
        self.app.logout().await.map_err(map_error)
    }

    async fn pause_sync(&self) -> zbus::fdo::Result<()> {
        self.app.pause_sync().await;
        Ok(())
    }

    async fn resume_sync(&self) -> zbus::fdo::Result<()> {
        self.app.resume_sync().await;
        Ok(())
    }

    async fn set_mount_path(&self, path: &str) -> zbus::fdo::Result<()> {
        self.app.set_mount_path(path).await.map_err(map_error)
    }

    async fn pin(&self, paths: Vec<String>) -> zbus::fdo::Result<()> {
        self.app.pin(&paths).await.map_err(map_error)
    }

    async fn evict(&self, paths: Vec<String>) -> zbus::fdo::Result<()> {
        self.app.evict(&paths).await.map_err(map_error)
    }

    async fn open_in_browser(&self, path: &str) -> zbus::fdo::Result<String> {
        self.app.open_in_browser(path).await.map_err(map_error)
    }

    async fn get_items(&self, paths: Vec<String>) -> zbus::fdo::Result<Vec<ItemSnapshot>> {
        self.app.get_items(&paths).await.map_err(map_error)
    }

    async fn get_status(&self) -> zbus::fdo::Result<StatusSnapshot> {
        self.app.get_status().await.map_err(map_error)
    }

    async fn get_status_json(&self) -> zbus::fdo::Result<String> {
        let status = self.app.get_status().await.map_err(map_error)?;
        serde_json::to_string(&status).map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
    }

    async fn retry_failed(&self) -> zbus::fdo::Result<()> {
        self.app.retry_failed().await;
        Ok(())
    }

    async fn get_items_json(&self, paths: Vec<String>) -> zbus::fdo::Result<String> {
        let items = self.app.get_items(&paths).await.map_err(map_error)?;
        serde_json::to_string(&items).map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
    }

    #[zbus(signal)]
    async fn auth_required(ctxt: &SignalContext<'_>, reason: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn sync_progress(ctxt: &SignalContext<'_>, status: StatusSnapshot) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn item_states_changed(
        ctxt: &SignalContext<'_>,
        items: Vec<ItemSnapshot>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn mount_state_changed(ctxt: &SignalContext<'_>, state: MountState) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn error_raised(ctxt: &SignalContext<'_>, message: &str) -> zbus::Result<()>;
}
