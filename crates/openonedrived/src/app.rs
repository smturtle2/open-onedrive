use crate::mount::MountController;
use anyhow::{Context, Result, bail};
use openonedrive_auth::build_authorization_request;
use openonedrive_config::{AppConfig, ProjectPaths, validate_mount_path};
use openonedrive_ipc_types::{AvailabilityState, ItemSnapshot, MountState, StatusSnapshot, SyncState};
use openonedrive_state::StateStore;
use openonedrive_vfs::{SnapshotHandle, VirtualEntry};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub struct OpenOneDriveApp {
    paths: ProjectPaths,
    config: RwLock<AppConfig>,
    state: StateStore,
    mount: tokio::sync::Mutex<MountController>,
    runtime: tokio::sync::RwLock<RuntimeStatus>,
}

#[derive(Debug, Clone)]
struct RuntimeStatus {
    sync_state: SyncState,
    mount_state: MountState,
    last_error: Option<String>,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            sync_state: SyncState::Starting,
            mount_state: MountState::Unmounted,
            last_error: None,
        }
    }
}

impl OpenOneDriveApp {
    pub async fn load() -> Result<Arc<Self>> {
        let paths = ProjectPaths::discover()?;
        paths.ensure()?;
        let config = AppConfig::load_or_create(&paths)?;
        let state = StateStore::open(&paths.db_file)?;
        let snapshot = SnapshotHandle::default();
        let app = Arc::new(Self {
            paths,
            config: RwLock::new(config),
            state,
            mount: tokio::sync::Mutex::new(MountController::new(snapshot)),
            runtime: tokio::sync::RwLock::new(RuntimeStatus::default()),
        });

        app.refresh_snapshot().await?;
        {
            let mut runtime = app.runtime.write().await;
            runtime.sync_state = SyncState::Idle;
        }
        Ok(app)
    }

    pub fn config(&self) -> AppConfig {
        self.config.read().expect("config lock poisoned").clone()
    }

    pub async fn startup_mount(&self) -> Result<()> {
        if let Some(path) = self.config().mount_path {
            self.mount_at(&path).await?;
        }
        Ok(())
    }

    pub async fn login(&self, client_id: &str) -> Result<String> {
        if client_id.trim().is_empty() {
            bail!("client ID cannot be empty");
        }

        let request = build_authorization_request(client_id, 53682)?;
        let mut config = self.config.write().expect("config lock poisoned");
        config.client_id = Some(client_id.trim().to_string());
        config.save(&self.paths)?;
        Ok(request.authorize_url)
    }

    pub async fn logout(&self) -> Result<()> {
        let mut config = self.config.write().expect("config lock poisoned");
        config.client_id = None;
        config.save(&self.paths)?;
        Ok(())
    }

    pub async fn pause_sync(&self) {
        let mut runtime = self.runtime.write().await;
        runtime.sync_state = SyncState::Paused;
    }

    pub async fn resume_sync(&self) {
        let mut runtime = self.runtime.write().await;
        runtime.sync_state = SyncState::Idle;
    }

    pub async fn set_mount_path(&self, raw_path: &str) -> Result<()> {
        let path = PathBuf::from(raw_path);
        validate_mount_path(&path)?;

        if !path.exists() {
            let parent = path.parent().context("mount path needs a parent directory")?;
            std::fs::create_dir_all(parent)
                .with_context(|| format!("unable to create {}", parent.display()))?;
            std::fs::create_dir_all(&path)
                .with_context(|| format!("unable to create {}", path.display()))?;
        }

        self.pause_sync().await;
        self.unmount().await;

        {
            let mut config = self.config.write().expect("config lock poisoned");
            config.mount_path = Some(path.clone());
            config.save(&self.paths)?;
        }

        self.mount_at(&path).await?;
        self.resume_sync().await;
        Ok(())
    }

    pub async fn pin(&self, paths: &[String]) -> Result<()> {
        let virtual_paths = self.normalize_virtual_paths(paths);
        self.state
            .set_availability(&virtual_paths, AvailabilityState::Pinned, true)?;
        self.refresh_snapshot().await?;
        Ok(())
    }

    pub async fn evict(&self, paths: &[String]) -> Result<()> {
        let virtual_paths = self.normalize_virtual_paths(paths);
        self.state
            .set_availability(&virtual_paths, AvailabilityState::OnlineOnly, false)?;
        self.refresh_snapshot().await?;
        Ok(())
    }

    pub async fn get_items(&self, paths: &[String]) -> Result<Vec<ItemSnapshot>> {
        let virtual_paths = self.normalize_virtual_paths(paths);
        let items = self.state.list_items_by_paths(&virtual_paths)?;
        Ok(items.into_iter().map(|item| item.to_snapshot()).collect())
    }

    pub async fn get_status(&self) -> Result<StatusSnapshot> {
        let config = self.config();
        let runtime = self.runtime.read().await.clone();
        Ok(StatusSnapshot {
            sync_state: runtime.sync_state,
            mount_state: runtime.mount_state,
            mount_path: config
                .mount_path
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            client_id_configured: config.client_id.is_some(),
            cache_limit_gb: config.cache_limit_gb,
            cache_usage_bytes: 0,
            items_indexed: self.state.items_indexed()?,
            last_error: runtime.last_error.unwrap_or_default(),
        })
    }

    pub async fn open_in_browser(&self, _path: &str) -> Result<String> {
        Ok(openonedrive_graph::GraphClient::browser_url().to_string())
    }

    pub async fn retry_failed(&self) {
        let mut runtime = self.runtime.write().await;
        runtime.last_error = None;
    }

    async fn refresh_snapshot(&self) -> Result<()> {
        let items = self.state.list_items()?;
        let entries: Vec<VirtualEntry> = items
            .into_iter()
            .map(|item| VirtualEntry {
                path: item.path,
                kind: item.kind,
                availability: item.availability,
                pinned: item.pinned,
                size: item.size,
                modified_unix: item.modified_unix,
                content_stub: item.content_stub,
            })
            .collect();
        let mount = self.mount.lock().await;
        mount.rebuild(&entries);
        Ok(())
    }

    fn normalize_virtual_paths(&self, paths: &[String]) -> Vec<String> {
        let mount_root = self.config().mount_path.unwrap_or_default();

        paths.iter()
            .map(|path| {
                let candidate = PathBuf::from(path);
                if mount_root.as_os_str().is_empty() || !candidate.is_absolute() {
                    return normalize_virtual_path_string(path);
                }

                if let Ok(stripped) = candidate.strip_prefix(&mount_root) {
                    let stripped = stripped.display().to_string();
                    if stripped.is_empty() {
                        "/".to_string()
                    } else {
                        normalize_virtual_path_string(&stripped)
                    }
                } else {
                    normalize_virtual_path_string(path)
                }
            })
            .collect()
    }

    async fn mount_at(&self, path: &Path) -> Result<()> {
        {
            let mut runtime = self.runtime.write().await;
            runtime.mount_state = MountState::Mounting;
            runtime.last_error = None;
        }

        let result = async {
            let mut mount = self.mount.lock().await;
            mount.mount(path)
        }
        .await;

        match result {
            Ok(()) => {
                let mut runtime = self.runtime.write().await;
                runtime.mount_state = MountState::Mounted;
                Ok(())
            }
            Err(error) => {
                let mut runtime = self.runtime.write().await;
                runtime.mount_state = MountState::Error;
                runtime.last_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    async fn unmount(&self) {
        let mut mount = self.mount.lock().await;
        mount.unmount();
        let mut runtime = self.runtime.write().await;
        runtime.mount_state = MountState::Unmounted;
    }
}

fn normalize_virtual_path_string(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "/".to_string();
    }
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}
