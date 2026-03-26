use crate::app::OpenOneDriveApp;
use openonedrive_ipc_types::{
    ConnectionState, FilesystemState, PathState, StatusSnapshot, SyncState,
};
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

    pub async fn emit_connection_state_changed(
        ctxt: &SignalContext<'_>,
        state: ConnectionState,
    ) -> zbus::Result<()> {
        Self::connection_state_changed(ctxt, state).await
    }

    pub async fn emit_filesystem_state_changed(
        ctxt: &SignalContext<'_>,
        state: FilesystemState,
    ) -> zbus::Result<()> {
        Self::filesystem_state_changed(ctxt, state).await
    }

    pub async fn emit_sync_state_changed(
        ctxt: &SignalContext<'_>,
        state: SyncState,
    ) -> zbus::Result<()> {
        Self::sync_state_changed(ctxt, state).await
    }

    pub async fn emit_auth_flow_started(ctxt: &SignalContext<'_>) -> zbus::Result<()> {
        Self::auth_flow_started(ctxt).await
    }

    pub async fn emit_auth_flow_completed(ctxt: &SignalContext<'_>) -> zbus::Result<()> {
        Self::auth_flow_completed(ctxt).await
    }

    pub async fn emit_error_raised(ctxt: &SignalContext<'_>, message: &str) -> zbus::Result<()> {
        Self::error_raised(ctxt, message).await
    }

    pub async fn emit_logs_updated(ctxt: &SignalContext<'_>) -> zbus::Result<()> {
        Self::logs_updated(ctxt).await
    }

    pub async fn emit_path_states_changed(
        ctxt: &SignalContext<'_>,
        paths: Vec<String>,
    ) -> zbus::Result<()> {
        Self::path_states_changed(ctxt, paths).await
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

    async fn repair_remote(&self) -> zbus::fdo::Result<()> {
        self.app.repair_remote().await.map_err(map_error)
    }

    async fn set_root_path(&self, path: &str) -> zbus::fdo::Result<()> {
        self.app.set_root_path(path).await.map_err(map_error)
    }

    async fn set_mount_path(&self, path: &str) -> zbus::fdo::Result<()> {
        self.app.set_mount_path(path).await.map_err(map_error)
    }

    async fn start_filesystem(&self) -> zbus::fdo::Result<()> {
        self.app.start_filesystem().await.map_err(map_error)
    }

    async fn stop_filesystem(&self) -> zbus::fdo::Result<()> {
        self.app.stop_filesystem().await.map_err(map_error)
    }

    async fn retry_filesystem(&self) -> zbus::fdo::Result<()> {
        self.app.retry_filesystem().await.map_err(map_error)
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

    async fn retry_transfer(&self, paths: Vec<String>) -> zbus::fdo::Result<u32> {
        self.app.retry_transfer(&paths).await.map_err(map_error)
    }

    async fn rescan(&self) -> zbus::fdo::Result<u32> {
        self.app.rescan().await.map_err(map_error)
    }

    async fn pause_sync(&self) -> zbus::fdo::Result<()> {
        self.app.pause_sync().await.map_err(map_error)
    }

    async fn resume_sync(&self) -> zbus::fdo::Result<()> {
        self.app.resume_sync().await.map_err(map_error)
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

    async fn get_path_states(&self, paths: Vec<String>) -> zbus::fdo::Result<Vec<PathState>> {
        self.app.get_path_states(&paths).await.map_err(map_error)
    }

    async fn get_path_states_json(&self, paths: Vec<String>) -> zbus::fdo::Result<String> {
        self.app
            .get_path_states_json(&paths)
            .await
            .map_err(map_error)
    }

    #[zbus(signal)]
    async fn connection_state_changed(
        ctxt: &SignalContext<'_>,
        state: ConnectionState,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn filesystem_state_changed(
        ctxt: &SignalContext<'_>,
        state: FilesystemState,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn sync_state_changed(ctxt: &SignalContext<'_>, state: SyncState) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn auth_flow_started(ctxt: &SignalContext<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn auth_flow_completed(ctxt: &SignalContext<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn error_raised(ctxt: &SignalContext<'_>, message: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn logs_updated(ctxt: &SignalContext<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn path_states_changed(ctxt: &SignalContext<'_>, paths: Vec<String>) -> zbus::Result<()>;
}
