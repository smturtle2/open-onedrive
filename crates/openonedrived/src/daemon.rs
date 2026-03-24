use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use open_onedrive_auth::AuthContext;
use open_onedrive_config::{AppConfig, AppPaths, validate_mount_path};
use open_onedrive_graph::GraphClient;
use open_onedrive_ipc_types::{
    INTERFACE_NAME, ItemSnapshot, OBJECT_PATH, SERVICE_NAME, ServiceStatus, SyncLifecycle,
};
use open_onedrive_state::StateStore;
use open_onedrive_vfs::{MountManager, validate_mount_ready};
use tokio::signal;
use tokio::sync::Mutex;
use tracing::{error, info};
use zbus::{Connection, connection::Builder, interface};

pub async fn run(print_config: bool) -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = AppConfig::load_or_create(&paths)?;
    if print_config {
        println!("{}", toml::to_string_pretty(&config)?);
        return Ok(());
    }

    let state = StateStore::open(&paths.state_db)?;
    let daemon = OpenOneDriveDaemon::new(paths, config, state);
    daemon.initialize_mount().await?;

    let _connection = Builder::session()?
        .name(SERVICE_NAME)?
        .serve_at(OBJECT_PATH, daemon.clone())?
        .build()
        .await
        .context("failed to publish D-Bus service")?;

    info!("open-onedrive daemon listening on {INTERFACE_NAME}");
    signal::ctrl_c().await?;
    Ok(())
}

#[derive(Clone)]
struct OpenOneDriveDaemon {
    inner: Arc<Mutex<DaemonState>>,
}

struct DaemonState {
    paths: AppPaths,
    config: AppConfig,
    state_store: StateStore,
    mount_manager: MountManager,
    graph_client: Option<GraphClient>,
}

impl OpenOneDriveDaemon {
    fn new(paths: AppPaths, config: AppConfig, state_store: StateStore) -> Self {
        let graph_client = config
            .client_id
            .clone()
            .and_then(|client_id| AuthContext::new(client_id).ok())
            .map(GraphClient::new);
        Self {
            inner: Arc::new(Mutex::new(DaemonState {
                paths,
                config,
                state_store,
                mount_manager: MountManager::new(),
                graph_client,
            })),
        }
    }

    async fn initialize_mount(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;
        inner.state_store.set_sync_state(SyncLifecycle::Idle)?;
        let items = inner.state_store.list_items()?;
        let path = inner.config.expanded_mount_path();
        if let Some(path) = path.as_ref() {
            if validate_mount_path(path).is_ok() && validate_mount_ready(path).is_ok() {
                if let Err(error) = inner.mount_manager.remount(Some(path), &items) {
                    error!("mount skipped: {error:#}");
                }
            }
        }
        Ok(())
    }
}

#[interface(name = INTERFACE_NAME, spawn = false)]
impl OpenOneDriveDaemon {
    async fn login(&self, client_id: String) -> zbus::fdo::Result<String> {
        let auth = AuthContext::new(client_id)
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        let mut inner = self.inner.lock().await;
        inner.config.client_id = Some(auth.client_id.clone());
        inner.graph_client = Some(GraphClient::new(auth.clone()));
        inner
            .config
            .persist(&inner.paths)
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        Ok(auth.login_hint())
    }

    async fn logout(&self) -> zbus::fdo::Result<()> {
        let mut inner = self.inner.lock().await;
        inner.config.client_id = None;
        inner.graph_client = None;
        inner
            .config
            .persist(&inner.paths)
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        Ok(())
    }

    async fn pause_sync(&self) -> zbus::fdo::Result<()> {
        let inner = self.inner.lock().await;
        inner
            .state_store
            .set_sync_state(SyncLifecycle::Paused)
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        Ok(())
    }

    async fn resume_sync(&self) -> zbus::fdo::Result<()> {
        let inner = self.inner.lock().await;
        inner
            .state_store
            .set_sync_state(SyncLifecycle::Idle)
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        Ok(())
    }

    async fn set_mount_path(&self, path: String) -> zbus::fdo::Result<()> {
        let requested = PathBuf::from(path);
        validate_mount_path(&requested)
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        validate_mount_ready(&requested)
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;

        let mut inner = self.inner.lock().await;
        inner.config.mount_path = Some(requested.clone());
        inner
            .config
            .persist(&inner.paths)
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        let items = inner
            .state_store
            .list_items()
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        inner
            .mount_manager
            .remount(Some(&requested), &items)
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        Ok(())
    }

    async fn pin(&self, paths: Vec<String>) -> zbus::fdo::Result<()> {
        self.update_items(paths, open_onedrive_ipc_types::ItemPresenceState::Pinned)
            .await
    }

    async fn evict(&self, paths: Vec<String>) -> zbus::fdo::Result<()> {
        self.update_items(paths, open_onedrive_ipc_types::ItemPresenceState::OnlineOnly)
            .await
    }

    async fn open_in_browser(&self, path: String) -> zbus::fdo::Result<String> {
        Ok(format!("TODO: open OneDrive web view for {path}"))
    }

    async fn retry_failed(&self) -> zbus::fdo::Result<()> {
        let inner = self.inner.lock().await;
        inner
            .state_store
            .set_sync_state(SyncLifecycle::Idle)
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        Ok(())
    }

    async fn get_items(&self, paths: Vec<String>) -> zbus::fdo::Result<Vec<ItemSnapshot>> {
        let inner = self.inner.lock().await;
        let mut items = inner
            .state_store
            .list_items()
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        if !paths.is_empty() {
            items.retain(|item| paths.iter().any(|path| path == &item.path));
        }
        Ok(items)
    }

    async fn get_status(&self) -> zbus::fdo::Result<ServiceStatus> {
        let inner = self.inner.lock().await;
        let mount_path = inner
            .config
            .expanded_mount_path()
            .unwrap_or_default()
            .display()
            .to_string();
        let mut status = inner
            .state_store
            .service_status(mount_path, inner.graph_client.is_some())
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        if let Some(graph) = inner.graph_client.as_ref() {
            status.account_label = graph.account_label();
        }
        Ok(status)
    }
}

impl OpenOneDriveDaemon {
    async fn update_items(
        &self,
        paths: Vec<String>,
        presence: open_onedrive_ipc_types::ItemPresenceState,
    ) -> zbus::fdo::Result<()> {
        if paths.is_empty() {
            bail_dbus("paths cannot be empty")?;
        }
        let mut inner = self.inner.lock().await;
        for path in paths {
            inner
                .state_store
                .upsert_item(&ItemSnapshot {
                    path,
                    size: 0,
                    kind: open_onedrive_ipc_types::ItemKind::File,
                    presence: presence.clone(),
                })
                .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        }
        Ok(())
    }
}

fn bail_dbus<T>(message: &str) -> zbus::fdo::Result<T> {
    Err(zbus::fdo::Error::Failed(message.to_string()))
}

