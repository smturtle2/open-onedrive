use crate::app::OpenOneDriveApp;
use openonedrive_ipc_types::{MountState, StatusSnapshot};
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
    async fn begin_connect(&self) -> zbus::fdo::Result<()> {
        self.app.begin_connect().await.map_err(map_error)
    }

    async fn disconnect(&self) -> zbus::fdo::Result<()> {
        self.app.disconnect().await.map_err(map_error)
    }

    async fn set_mount_path(&self, path: &str) -> zbus::fdo::Result<()> {
        self.app.set_mount_path(path).await.map_err(map_error)
    }

    async fn mount(&self) -> zbus::fdo::Result<()> {
        self.app.mount().await.map_err(map_error)
    }

    async fn unmount(&self) -> zbus::fdo::Result<()> {
        self.app.unmount().await.map_err(map_error)
    }

    async fn retry_mount(&self) -> zbus::fdo::Result<()> {
        self.app.retry_mount().await.map_err(map_error)
    }

    async fn keep_local(&self, paths: Vec<String>) -> zbus::fdo::Result<u32> {
        self.app.keep_local(&paths).await.map_err(map_error)
    }

    async fn make_online_only(&self, paths: Vec<String>) -> zbus::fdo::Result<u32> {
        self.app.make_online_only(&paths).await.map_err(map_error)
    }

    async fn get_status(&self) -> zbus::fdo::Result<StatusSnapshot> {
        self.app.get_status().await.map_err(map_error)
    }

    async fn get_status_json(&self) -> zbus::fdo::Result<String> {
        self.app.get_status_json().await.map_err(map_error)
    }

    async fn get_recent_log_lines(&self, limit: u32) -> zbus::fdo::Result<Vec<String>> {
        self.app
            .get_recent_log_lines(limit as usize)
            .await
            .map_err(map_error)
    }

    #[zbus(signal)]
    async fn mount_state_changed(ctxt: &SignalContext<'_>, state: MountState) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn auth_flow_started(ctxt: &SignalContext<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn auth_flow_completed(ctxt: &SignalContext<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn error_raised(ctxt: &SignalContext<'_>, message: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn logs_updated(ctxt: &SignalContext<'_>) -> zbus::Result<()>;
}
