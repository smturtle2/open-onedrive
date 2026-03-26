mod path_state;
mod vfs;

use anyhow::{Context, Result, anyhow, bail};
use openonedrive_config::{AppConfig, ProjectPaths, validate_root_path};
use openonedrive_ipc_types::{
    ConnectionState, FilesystemState, PathState, PathSyncState, StatusSnapshot, SyncState,
};
use openonedrive_state::{RuntimeState, StateStore};
use path_state::PathStateStore;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock, broadcast};
use tracing::warn;
use vfs::{OpenOneDriveFs, OpenRequest, Provider, SnapshotHandle, VirtualEntry};

const MAX_RECENT_LOGS: usize = 200;
const RESCAN_INTERVAL: Duration = Duration::from_secs(120);
pub const BACKEND_NAME: &str = "custom-fuse-rclone";
const LEGACY_ONEDRIVE_DRIVE_METADATA_ERROR: &str = "unable to get drive_id and drive_type";
const RCLONE_ONEDRIVE_DEFAULT_CLIENT_ID: &str = "b15665d9-eda6-4092-8539-0eec376afd59";
const RCLONE_ONEDRIVE_DEFAULT_CLIENT_SECRET_OBSCURED: &str =
    "_JUdzh3LnKNqSPcf4Wu5fgMFIQOI8glZu_akYgR8yf6egowNBg-R";

#[derive(Debug, Clone)]
pub enum BackendEvent {
    ConnectionStateChanged,
    FilesystemStateChanged,
    SyncStateChanged,
    AuthFlowStarted,
    AuthFlowCompleted,
    ErrorRaised(String),
    LogsUpdated,
    PathStatesChanged(Vec<String>),
}

#[derive(Debug, Clone)]
struct Runtime {
    remote_configured: bool,
    connection_state: ConnectionState,
    filesystem_state: FilesystemState,
    sync_state: SyncState,
    last_error: String,
    last_sync_error: String,
    last_log_line: String,
    pinned_relative_paths: BTreeSet<String>,
    conflict_relative_paths: BTreeSet<String>,
    rclone_version: String,
    pending_downloads: u32,
    pending_uploads: u32,
    last_sync_at: u64,
    sync_paused: bool,
}

impl Runtime {
    fn from_state(state: RuntimeState, remote_configured: bool) -> Self {
        let connection_state = if remote_configured {
            match state.connection_state {
                ConnectionState::Disconnected => ConnectionState::Ready,
                ConnectionState::Connecting => ConnectionState::Connecting,
                ConnectionState::Ready | ConnectionState::Error => state.connection_state,
            }
        } else {
            ConnectionState::Disconnected
        };

        Self {
            remote_configured,
            connection_state,
            filesystem_state: match state.filesystem_state {
                FilesystemState::Running | FilesystemState::Starting => FilesystemState::Stopped,
                other => other,
            },
            sync_state: if state.sync_paused {
                SyncState::Paused
            } else {
                state.sync_state
            },
            last_error: state.last_error,
            last_sync_error: state.last_sync_error,
            last_log_line: state.last_log_line,
            pinned_relative_paths: state.pinned_relative_paths.into_iter().collect(),
            conflict_relative_paths: state.conflict_relative_paths.into_iter().collect(),
            rclone_version: String::new(),
            pending_downloads: state.pending_downloads,
            pending_uploads: state.pending_uploads,
            last_sync_at: state.last_sync_at,
            sync_paused: state.sync_paused,
        }
    }
}

#[derive(Debug, Clone)]
struct RemoteConfigSection {
    options: BTreeMap<String, String>,
}

impl RemoteConfigSection {
    fn is_onedrive(&self) -> bool {
        self.option("type")
            .is_some_and(|value| value.eq_ignore_ascii_case("onedrive"))
    }

    fn missing_drive_metadata(&self) -> bool {
        self.option("drive_id").is_none() || self.option("drive_type").is_none()
    }

    fn option(&self, key: &str) -> Option<&str> {
        self.options
            .get(key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Deserialize)]
struct RefreshedAccessToken {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct DriveMetadata {
    #[serde(rename = "id")]
    drive_id: String,
    #[serde(rename = "driveType")]
    drive_type: String,
}

pub struct RcloneBackend {
    paths: ProjectPaths,
    config: RwLock<AppConfig>,
    state_store: StateStore,
    path_state_store: PathStateStore,
    runtime: RwLock<Runtime>,
    recent_logs: std::sync::Mutex<VecDeque<String>>,
    connect_child: Mutex<Option<Child>>,
    connect_generation: Mutex<u64>,
    rescan_generation: Mutex<u64>,
    filesystem_session: std::sync::Mutex<Option<fuser::BackgroundSession>>,
    underlay_root: std::sync::Mutex<Option<File>>,
    snapshot: SnapshotHandle,
    runtime_handle: tokio::runtime::Handle,
    event_tx: broadcast::Sender<BackendEvent>,
}

impl RcloneBackend {
    pub async fn load(paths: ProjectPaths, config: AppConfig) -> Result<Arc<Self>> {
        paths.ensure()?;
        let state_store = StateStore::open(&paths.runtime_state_file)?;
        let path_state_store = PathStateStore::open(&paths.path_state_db_file)?;
        let persisted = state_store.load()?;
        let remote_configured = has_remote_config(&paths.rclone_config_file, &config.remote_name)?;
        let (event_tx, _) = broadcast::channel(64);

        let backend = Arc::new(Self {
            paths,
            config: RwLock::new(config),
            state_store,
            path_state_store,
            runtime: RwLock::new(Runtime::from_state(persisted, remote_configured)),
            recent_logs: std::sync::Mutex::new(VecDeque::with_capacity(MAX_RECENT_LOGS)),
            connect_child: Mutex::new(None),
            connect_generation: Mutex::new(0),
            rescan_generation: Mutex::new(0),
            filesystem_session: std::sync::Mutex::new(None),
            underlay_root: std::sync::Mutex::new(None),
            snapshot: SnapshotHandle::default(),
            runtime_handle: tokio::runtime::Handle::current(),
            event_tx,
        });

        backend.refresh_rclone_version().await;
        backend.refresh_virtual_snapshot()?;

        Ok(backend)
    }

    pub async fn bootstrap(self: &Arc<Self>) -> Result<()> {
        self.refresh_rclone_version().await;
        self.reconcile_remote_state_from_disk().await?;
        if !self.runtime.read().await.remote_configured {
            return Ok(());
        }

        if let Err(error) = self.ensure_remote_ready_for_use().await {
            self.record_connection_error(error.to_string()).await;
            return Ok(());
        }

        if self.current_remote_needs_repair().await? {
            return Ok(());
        }

        let config = self.current_config().await;
        if config.auto_start_filesystem {
            if let Err(error) = self.start_filesystem().await {
                self.record_error(error.to_string()).await;
            }
        } else if !self.runtime.read().await.sync_paused {
            self.spawn_rescan("startup");
            self.restart_rescan_loop().await;
        }

        Ok(())
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<BackendEvent> {
        self.event_tx.subscribe()
    }

    pub async fn current_config(&self) -> AppConfig {
        self.config.read().await.clone()
    }

    fn runtime_read_guard(&self) -> tokio::sync::RwLockReadGuard<'_, Runtime> {
        if let Ok(runtime) = self.runtime.try_read() {
            return runtime;
        }

        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.runtime_handle.block_on(self.runtime.read()))
        } else {
            self.runtime_handle.block_on(self.runtime.read())
        }
    }

    fn runtime_write_guard(&self) -> tokio::sync::RwLockWriteGuard<'_, Runtime> {
        if let Ok(runtime) = self.runtime.try_write() {
            return runtime;
        }

        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.runtime_handle.block_on(self.runtime.write()))
        } else {
            self.runtime_handle.block_on(self.runtime.write())
        }
    }

    fn config_read_guard(&self) -> tokio::sync::RwLockReadGuard<'_, AppConfig> {
        if let Ok(config) = self.config.try_read() {
            return config;
        }

        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.runtime_handle.block_on(self.config.read()))
        } else {
            self.runtime_handle.block_on(self.config.read())
        }
    }

    fn persist_runtime_blocking(&self) -> Result<()> {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.runtime_handle.block_on(self.persist_runtime()))
        } else {
            self.runtime_handle.block_on(self.persist_runtime())
        }
    }

    pub async fn get_path_states(&self, raw_paths: &[String]) -> Result<Vec<PathState>> {
        let config = self.current_config().await;
        let mut relative_paths = Vec::with_capacity(raw_paths.len());
        for raw_path in raw_paths {
            let relative = relative_path_for(&config.root_path, Path::new(raw_path))?;
            relative_paths.push(relative_string(&relative));
        }
        self.path_state_store.get_many(&relative_paths)
    }

    pub async fn get_path_states_json(&self, raw_paths: &[String]) -> Result<String> {
        serde_json::to_string(&self.get_path_states(raw_paths).await?)
            .context("unable to serialize path states")
    }

    pub async fn set_root_path(self: &Arc<Self>, raw_path: &str) -> Result<()> {
        let requested_path = PathBuf::from(raw_path);
        let mut updated_config = self.current_config().await;
        let previous_root_path = updated_config.root_path.clone();
        validate_root_path(&requested_path, &updated_config.backing_dir_name)?;

        let should_restart = self.runtime.read().await.filesystem_state == FilesystemState::Running;
        self.stop_filesystem().await?;

        if !requested_path.exists() {
            fs::create_dir_all(&requested_path)
                .with_context(|| format!("unable to create {}", requested_path.display()))?;
        }
        migrate_backing_root(
            &previous_root_path,
            &requested_path,
            &updated_config.backing_dir_name,
        )?;
        updated_config.root_path = requested_path;
        updated_config.save(&self.paths)?;
        *self.config.write().await = updated_config;

        if should_restart && self.runtime.read().await.remote_configured {
            self.start_filesystem().await?;
        } else {
            self.persist_runtime().await?;
        }
        Ok(())
    }

    pub async fn set_mount_path(self: &Arc<Self>, raw_path: &str) -> Result<()> {
        self.set_root_path(raw_path).await
    }

    pub async fn begin_connect(self: &Arc<Self>) -> Result<()> {
        self.reconcile_remote_state_from_disk().await?;
        if self.runtime.read().await.remote_configured {
            return Ok(());
        }

        self.stop_connect_process().await?;

        let config = self.current_config().await;
        let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
        let mut command = Command::new(binary);
        command
            .args(build_connect_args(&config, &self.paths))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .context("failed to spawn rclone config create")?;
        let generation = {
            let mut generation = self.connect_generation.lock().await;
            *generation += 1;
            *generation
        };

        {
            let mut runtime = self.runtime.write().await;
            runtime.connection_state = ConnectionState::Connecting;
            runtime.last_error.clear();
        }
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::ConnectionStateChanged);
        self.emit_event(BackendEvent::AuthFlowStarted);

        if let Some(stdout) = child.stdout.take() {
            self.spawn_log_reader(stdout, "connect stdout");
        }
        if let Some(stderr) = child.stderr.take() {
            self.spawn_log_reader(stderr, "connect stderr");
        }

        *self.connect_child.lock().await = Some(child);

        let backend = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;

                let exit = {
                    let mut slot = backend.connect_child.lock().await;
                    match slot.as_mut() {
                        Some(child) => child.try_wait(),
                        None => return,
                    }
                };

                let current_generation = *backend.connect_generation.lock().await;
                if generation != current_generation {
                    return;
                }

                let exit = match exit {
                    Ok(Some(status)) => {
                        let mut slot = backend.connect_child.lock().await;
                        slot.take();
                        Ok(status)
                    }
                    Ok(None) => continue,
                    Err(error) => {
                        let mut slot = backend.connect_child.lock().await;
                        slot.take();
                        Err(error)
                    }
                };

                match exit {
                    Ok(status) if status.success() => {
                        let config = backend.current_config().await;
                        if let Err(error) = backend.complete_connect(config, None).await {
                            backend.record_connection_error(error.to_string()).await;
                        }
                    }
                    Ok(status) => {
                        let config = backend.current_config().await;
                        if let Err(error) = backend
                            .complete_connect(
                                config,
                                Some(format!("rclone config create exited with status {status}")),
                            )
                            .await
                        {
                            backend.record_connection_error(error.to_string()).await;
                        }
                    }
                    Err(error) => {
                        backend
                            .record_connection_error(format!(
                                "waiting for rclone connect failed: {error}"
                            ))
                            .await;
                    }
                }
                return;
            }
        });

        Ok(())
    }

    pub async fn repair_remote(self: &Arc<Self>) -> Result<()> {
        self.prepare_remote_repair().await?;
        self.begin_connect().await
    }

    pub async fn disconnect(self: &Arc<Self>) -> Result<()> {
        self.stop_connect_process().await?;
        self.stop_rescan_loop().await;
        self.stop_filesystem().await?;
        remove_file_if_exists(&self.paths.rclone_config_file)?;
        self.clear_backing_root()?;

        {
            let mut runtime = self.runtime.write().await;
            runtime.remote_configured = false;
            runtime.connection_state = ConnectionState::Disconnected;
            runtime.filesystem_state = FilesystemState::Stopped;
            runtime.sync_state = SyncState::Idle;
            runtime.last_error.clear();
            runtime.last_sync_error.clear();
            runtime.last_log_line.clear();
            runtime.pinned_relative_paths.clear();
            runtime.conflict_relative_paths.clear();
            runtime.pending_downloads = 0;
            runtime.pending_uploads = 0;
            runtime.last_sync_at = 0;
            runtime.sync_paused = false;
        }
        self.recent_logs.lock().expect("logs poisoned").clear();
        self.path_state_store.clear()?;
        self.refresh_virtual_snapshot()?;
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::ConnectionStateChanged);
        self.emit_event(BackendEvent::FilesystemStateChanged);
        self.emit_event(BackendEvent::SyncStateChanged);
        self.emit_event(BackendEvent::PathStatesChanged(Vec::new()));
        Ok(())
    }

    pub async fn start_filesystem(self: &Arc<Self>) -> Result<()> {
        if let Err(error) = self.ensure_remote_ready_for_use().await {
            let message = error.to_string();
            self.record_connection_error(message.clone()).await;
            bail!(message);
        }
        {
            let runtime = self.runtime.read().await;
            if !runtime.remote_configured {
                bail!("no OneDrive remote is configured yet");
            }
            if matches!(
                runtime.filesystem_state,
                FilesystemState::Running | FilesystemState::Starting
            ) {
                return Ok(());
            }
        }

        let config = self.current_config().await;
        if !config.root_path.exists() {
            fs::create_dir_all(&config.root_path)
                .with_context(|| format!("unable to create {}", config.root_path.display()))?;
        }
        fs::create_dir_all(config.backing_dir_path())
            .with_context(|| format!("unable to create {}", config.backing_dir_path().display()))?;
        validate_root_path(&config.root_path, &config.backing_dir_name)?;

        {
            let mut runtime = self.runtime.write().await;
            runtime.filesystem_state = FilesystemState::Starting;
            runtime.last_error.clear();
        }
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::FilesystemStateChanged);
        let result: Result<()> = async {
            if !self.runtime.read().await.sync_paused {
                self.rescan().await?;
            } else {
                self.refresh_virtual_snapshot()?;
            }

            let root_handle = File::open(&config.root_path)
                .with_context(|| format!("unable to open {}", config.root_path.display()))?;
            *self.underlay_root.lock().expect("underlay root poisoned") = Some(root_handle);

            let provider: Arc<dyn Provider> = Arc::new(FuseBridge {
                backend: Arc::downgrade(self),
            });
            let session = OpenOneDriveFs::mount(self.snapshot.clone(), provider, &config.root_path)
                .with_context(|| {
                    format!(
                        "unable to mount filesystem at {}",
                        config.root_path.display()
                    )
                })?;
            *self
                .filesystem_session
                .lock()
                .expect("filesystem session poisoned") = Some(session);

            {
                let mut runtime = self.runtime.write().await;
                runtime.filesystem_state = FilesystemState::Running;
                runtime.connection_state = ConnectionState::Ready;
                runtime.last_error.clear();
            }
            self.persist_runtime().await?;
            self.emit_event(BackendEvent::FilesystemStateChanged);
            self.emit_event(BackendEvent::ConnectionStateChanged);
            if !self.runtime.read().await.sync_paused {
                self.restart_rescan_loop().await;
            }
            Ok(())
        }
        .await;

        if let Err(error) = &result {
            self.fail_filesystem_start(error.to_string()).await?;
        }

        result
    }

    pub async fn mount(self: &Arc<Self>) -> Result<()> {
        self.start_filesystem().await
    }

    pub async fn stop_filesystem(self: &Arc<Self>) -> Result<()> {
        self.filesystem_session
            .lock()
            .expect("filesystem session poisoned")
            .take();
        self.underlay_root
            .lock()
            .expect("underlay root poisoned")
            .take();

        {
            let mut runtime = self.runtime.write().await;
            runtime.filesystem_state = FilesystemState::Stopped;
            if runtime.remote_configured {
                runtime.connection_state = ConnectionState::Ready;
            } else {
                runtime.connection_state = ConnectionState::Disconnected;
            }
            runtime.last_error.clear();
        }
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::FilesystemStateChanged);
        self.emit_event(BackendEvent::ConnectionStateChanged);
        Ok(())
    }

    pub async fn unmount(self: &Arc<Self>) -> Result<()> {
        self.stop_filesystem().await
    }

    pub async fn retry_filesystem(self: &Arc<Self>) -> Result<()> {
        self.stop_filesystem().await?;
        self.start_filesystem().await
    }

    pub async fn retry_mount(self: &Arc<Self>) -> Result<()> {
        self.retry_filesystem().await
    }

    pub async fn rescan(self: &Arc<Self>) -> Result<u32> {
        if let Err(error) = self.ensure_remote_ready_for_use().await {
            let message = error.to_string();
            self.record_connection_error(message.clone()).await;
            bail!(message);
        }
        {
            let runtime = self.runtime.read().await;
            if !runtime.remote_configured {
                bail!("configure OneDrive before scanning remote state");
            }
        }

        self.begin_sync_activity(SyncState::Scanning)?;
        let result: Result<u32> = async {
            let entries = self.scan_remote_entries().await?;
            let snapshot = self.build_snapshot_from_remote_entries(&entries)?;
            let store = self.path_state_store.clone();
            let snapshot_for_store = snapshot.clone();
            tokio::task::spawn_blocking(move || store.replace_all(&snapshot_for_store))
                .await
                .context("path-state write task join failed")??;
            self.refresh_virtual_snapshot_with_states(&snapshot);
            self.sync_runtime_sets_from_states(&snapshot)?;
            self.complete_sync_activity(None).await?;
            self.emit_event(BackendEvent::PathStatesChanged(Vec::new()));
            Ok(snapshot.len() as u32)
        }
        .await;

        if let Err(error) = &result {
            self.complete_sync_activity(Some(error.to_string())).await?;
        }

        result
    }

    pub async fn pause_sync(&self) -> Result<()> {
        self.stop_rescan_loop().await;
        {
            let mut runtime = self.runtime.write().await;
            runtime.sync_paused = true;
            runtime.sync_state = SyncState::Paused;
        }
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::SyncStateChanged);
        Ok(())
    }

    pub async fn resume_sync(self: &Arc<Self>) -> Result<()> {
        {
            let mut runtime = self.runtime.write().await;
            runtime.sync_paused = false;
            if runtime.sync_state == SyncState::Paused {
                runtime.sync_state = SyncState::Idle;
            }
        }
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::SyncStateChanged);
        self.restart_rescan_loop().await;
        self.enqueue_dirty_uploads()?;
        self.spawn_rescan("resume");
        Ok(())
    }

    pub async fn keep_local(self: &Arc<Self>, raw_paths: &[String]) -> Result<u32> {
        let config = self.current_config().await;
        let selected_paths = expand_selected_paths(&config.root_path, raw_paths)?;
        if selected_paths.is_empty() {
            bail!("select at least one file or directory inside the OneDrive folder");
        }

        self.begin_sync_activity(SyncState::Syncing)?;
        let mut changed = Vec::new();
        let result = (|| -> Result<u32> {
            for relative_path in &selected_paths {
                self.hydrate_relative_path_sync(relative_path)?;
                changed.push(relative_path.clone());
            }
            self.set_pinned_state(&changed, true)?;
            self.rebuild_path_state_snapshot_sync()?;
            Ok(changed.len() as u32)
        })();

        match result {
            Ok(count) => {
                self.complete_sync_activity(None).await?;
                self.append_log(format!("kept {count} item(s) available on this device"));
                self.emit_path_state_refresh(&changed);
                Ok(count)
            }
            Err(error) => {
                if !changed.is_empty() {
                    let _ = self.rebuild_path_state_snapshot_sync();
                    self.emit_path_state_refresh(&changed);
                }
                self.complete_sync_activity(Some(error.to_string())).await?;
                Err(error)
            }
        }
    }

    pub async fn make_online_only(self: &Arc<Self>, raw_paths: &[String]) -> Result<u32> {
        let config = self.current_config().await;
        let selected_paths = expand_selected_paths(&config.root_path, raw_paths)?;
        if selected_paths.is_empty() {
            bail!("select at least one file or directory inside the OneDrive folder");
        }

        self.begin_sync_activity(SyncState::Syncing)?;
        let mut changed = Vec::new();
        let result = (|| -> Result<u32> {
            for relative_path in &selected_paths {
                self.evict_relative_path_sync(relative_path)?;
                changed.push(relative_path.clone());
            }
            self.set_pinned_state(&changed, false)?;
            self.rebuild_path_state_snapshot_sync()?;
            Ok(changed.len() as u32)
        })();

        match result {
            Ok(count) => {
                self.complete_sync_activity(None).await?;
                self.append_log(format!("returned {count} item(s) to online-only mode"));
                self.emit_path_state_refresh(&changed);
                Ok(count)
            }
            Err(error) => {
                if !changed.is_empty() {
                    let _ = self.rebuild_path_state_snapshot_sync();
                    self.emit_path_state_refresh(&changed);
                }
                self.complete_sync_activity(Some(error.to_string())).await?;
                Err(error)
            }
        }
    }

    pub async fn retry_transfer(self: &Arc<Self>, raw_paths: &[String]) -> Result<u32> {
        let config = self.current_config().await;
        let states = self.path_state_store.all()?;
        let relative_paths = expand_retry_paths(&config.root_path, raw_paths, &states)?;
        if relative_paths.is_empty() {
            bail!("select at least one file inside the OneDrive folder");
        }
        for relative_path in &relative_paths {
            self.enqueue_upload(relative_path.clone(), true);
        }
        Ok(relative_paths.len() as u32)
    }

    pub async fn status(&self) -> Result<StatusSnapshot> {
        if self.runtime.read().await.rclone_version.is_empty() {
            self.refresh_rclone_version().await;
        }

        self.reconcile_remote_state_from_disk().await?;
        let runtime = self.runtime.read().await.clone();
        let config = self.current_config().await;
        Ok(StatusSnapshot {
            backend: BACKEND_NAME.to_string(),
            remote_configured: runtime.remote_configured,
            needs_remote_repair: remote_config_needs_repair(
                &self.paths.rclone_config_file,
                &config.remote_name,
            )?,
            connection_state: runtime.connection_state,
            filesystem_state: runtime.filesystem_state,
            sync_state: runtime.sync_state,
            root_path: config.root_path.display().to_string(),
            backing_dir_name: config.backing_dir_name.clone(),
            backing_usage_bytes: directory_size_bytes(&self.backing_root_access_path()?)?,
            pinned_file_count: runtime.pinned_relative_paths.len() as u32,
            pending_downloads: runtime.pending_downloads,
            pending_uploads: runtime.pending_uploads,
            conflict_count: runtime.conflict_relative_paths.len() as u32,
            last_sync_at: runtime.last_sync_at,
            last_sync_error: runtime.last_sync_error,
            rclone_version: runtime.rclone_version,
            last_error: runtime.last_error,
            last_log_line: runtime.last_log_line,
            custom_client_id_configured: config.custom_client_id.is_some(),
        })
    }

    pub async fn recent_log_lines(&self, limit: usize) -> Vec<String> {
        let logs = self.recent_logs.lock().expect("logs poisoned");
        let skip = logs.len().saturating_sub(limit);
        logs.iter().skip(skip).cloned().collect()
    }

    async fn current_remote_needs_repair(&self) -> Result<bool> {
        let config = self.current_config().await;
        remote_config_needs_repair(&self.paths.rclone_config_file, &config.remote_name)
    }

    async fn ensure_remote_ready_for_use(&self) -> Result<()> {
        let config = self.current_config().await;
        if !has_remote_config(&self.paths.rclone_config_file, &config.remote_name)? {
            return Ok(());
        }

        let paths = self.paths.clone();
        let remote_name = config.remote_name.clone();
        let rclone_bin = config.rclone_bin.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            migrate_legacy_onedrive_remote(
                &paths.rclone_config_file,
                &remote_name,
                rclone_bin.as_deref(),
            )
        })
        .await
        .context("legacy remote migration task join failed")??;

        if outcome {
            self.append_log(
                "updated legacy OneDrive remote metadata without clearing local state".into(),
            );
        }

        Ok(())
    }

    async fn prepare_remote_repair(self: &Arc<Self>) -> Result<()> {
        self.stop_connect_process().await?;
        self.stop_rescan_loop().await;
        self.stop_filesystem().await?;
        remove_file_if_exists(&self.paths.rclone_config_file)?;

        {
            let mut runtime = self.runtime.write().await;
            runtime.remote_configured = false;
            runtime.connection_state = ConnectionState::Disconnected;
            runtime.filesystem_state = FilesystemState::Stopped;
            runtime.sync_state = if runtime.sync_paused {
                SyncState::Paused
            } else {
                SyncState::Idle
            };
            runtime.last_error.clear();
            runtime.last_sync_error.clear();
        }
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::ConnectionStateChanged);
        self.emit_event(BackendEvent::FilesystemStateChanged);
        self.emit_event(BackendEvent::SyncStateChanged);
        Ok(())
    }

    fn begin_sync_activity(&self, sync_state: SyncState) -> Result<()> {
        let mut runtime = self.runtime_write_guard();
        runtime.sync_state = sync_state;
        runtime.last_sync_error.clear();
        drop(runtime);
        self.persist_runtime_blocking()?;
        self.emit_event(BackendEvent::SyncStateChanged);
        Ok(())
    }

    fn refresh_virtual_snapshot_with_states(&self, states: &[PathState]) {
        let entries = states
            .iter()
            .map(|state| VirtualEntry {
                path: state.path.clone(),
                is_dir: state.is_dir,
                size_bytes: state.size_bytes,
                modified_unix: state.last_sync_at,
            })
            .collect::<Vec<_>>();
        self.snapshot.rebuild(&entries);
    }

    fn sync_runtime_sets_from_states(&self, states: &[PathState]) -> Result<()> {
        let mut runtime = self.runtime_write_guard();
        runtime.pinned_relative_paths = states
            .iter()
            .filter(|state| !state.is_dir && state.pinned)
            .map(|state| state.path.clone())
            .collect();
        runtime.conflict_relative_paths = states
            .iter()
            .filter(|state| !state.is_dir && state.state == PathSyncState::Conflict)
            .map(|state| state.path.clone())
            .collect();
        drop(runtime);
        self.persist_runtime_blocking()?;
        Ok(())
    }

    fn rebuild_path_state_snapshot_sync(&self) -> Result<()> {
        let normalized = normalize_path_state_snapshot(self.path_state_store.all()?);
        self.path_state_store.replace_all(&normalized)?;
        self.refresh_virtual_snapshot_with_states(&normalized);
        self.sync_runtime_sets_from_states(&normalized)?;
        Ok(())
    }

    fn emit_path_state_refresh(&self, paths: &[String]) {
        self.emit_event(BackendEvent::PathStatesChanged(affected_relative_paths(
            paths,
        )));
    }

    async fn complete_sync_activity(&self, error: Option<String>) -> Result<()> {
        let mut error_message = None;
        {
            let mut runtime = self.runtime.write().await;
            if let Some(error) = error {
                runtime.sync_state = SyncState::Error;
                runtime.last_sync_error = error.clone();
                error_message = Some(error);
            } else {
                runtime.sync_state = if runtime.sync_paused {
                    SyncState::Paused
                } else {
                    SyncState::Idle
                };
                runtime.last_sync_error.clear();
                runtime.last_sync_at = unix_timestamp();
            }
        }
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::SyncStateChanged);
        if let Some(message) = error_message {
            self.emit_event(BackendEvent::ErrorRaised(message));
        }
        Ok(())
    }

    fn set_pinned_state(&self, relative_paths: &[String], pinned: bool) -> Result<()> {
        if relative_paths.is_empty() {
            return Ok(());
        }
        let mut states = self.path_state_store.get_many(relative_paths)?;
        let snapshot = self.path_state_store.all()?;
        let snapshot_map = snapshot
            .into_iter()
            .map(|state| (state.path.clone(), state))
            .collect::<HashMap<_, _>>();

        for relative_path in relative_paths {
            let mut state = states
                .iter()
                .find(|state| &state.path == relative_path)
                .cloned()
                .or_else(|| snapshot_map.get(relative_path).cloned())
                .unwrap_or_else(|| PathState {
                    path: relative_path.clone(),
                    is_dir: false,
                    state: if pinned {
                        PathSyncState::PinnedLocal
                    } else {
                        PathSyncState::OnlineOnly
                    },
                    size_bytes: 0,
                    pinned,
                    hydrated: pinned,
                    dirty: false,
                    error: String::new(),
                    last_sync_at: unix_timestamp(),
                    base_revision: String::new(),
                    conflict_reason: String::new(),
                });
            state.pinned = pinned;
            if !state.is_dir {
                state.state = derive_path_state(&state);
            }
            state.last_sync_at = unix_timestamp();
            states.push(state);
        }
        self.path_state_store.upsert_many(&dedup_states(states))?;
        Ok(())
    }

    fn backing_root_access_path(&self) -> Result<PathBuf> {
        let config = self.config_read_guard().clone();
        if let Some(root) = self
            .underlay_root
            .lock()
            .expect("underlay root poisoned")
            .as_ref()
        {
            use std::os::fd::AsRawFd;
            return Ok(PathBuf::from(format!(
                "/proc/self/fd/{}/{}",
                root.as_raw_fd(),
                config.backing_dir_name
            )));
        }
        Ok(config.backing_dir_path())
    }

    fn backing_file_path(&self, relative_path: &str) -> Result<PathBuf> {
        Ok(self.backing_root_access_path()?.join(relative_path))
    }

    fn ensure_backing_parent(&self, relative_path: &str) -> Result<PathBuf> {
        let path = self.backing_file_path(relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("unable to create {}", parent.display()))?;
        }
        Ok(path)
    }

    fn refresh_virtual_snapshot(&self) -> Result<()> {
        let states = self.path_state_store.all()?;
        self.refresh_virtual_snapshot_with_states(&states);
        Ok(())
    }

    async fn scan_remote_entries(&self) -> Result<Vec<RcloneListEntry>> {
        let config = self.current_config().await;
        let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
        let output = Command::new(binary)
            .args(build_lsjson_args(&config, &self.paths))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("failed to execute rclone lsjson")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if is_legacy_onedrive_remote_error(&stderr) {
                bail!("{}", build_remote_repair_message(&stderr));
            }
            bail!(
                "rclone lsjson failed{}",
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {stderr}")
                }
            );
        }

        let payload =
            String::from_utf8(output.stdout).context("rclone lsjson returned invalid utf-8")?;
        serde_json::from_str::<Vec<RcloneListEntry>>(&payload)
            .context("unable to parse rclone lsjson output")
    }

    fn build_snapshot_from_remote_entries(
        &self,
        entries: &[RcloneListEntry],
    ) -> Result<Vec<PathState>> {
        let existing = self
            .path_state_store
            .all()?
            .into_iter()
            .map(|state| (state.path.clone(), state))
            .collect::<HashMap<_, _>>();
        let mut states = BTreeMap::<String, PathState>::new();

        for entry in entries {
            if entry.path.is_empty() {
                continue;
            }

            let existing_state = existing.get(&entry.path);
            let hydrated = if entry.is_dir {
                false
            } else {
                self.backing_file_path(&entry.path)
                    .ok()
                    .is_some_and(|path| path.exists())
            };
            let pinned = existing_state.is_some_and(|state| state.pinned);
            let dirty = existing_state.is_some_and(|state| state.dirty);
            let error = existing_state
                .map(|state| state.error.clone())
                .unwrap_or_default();
            let conflict_reason = existing_state
                .map(|state| state.conflict_reason.clone())
                .unwrap_or_default();
            let modified_unix = if dirty {
                existing_state
                    .map(|state| state.last_sync_at)
                    .unwrap_or_else(unix_timestamp)
            } else {
                parse_rclone_mod_time_unix(&entry.mod_time)
                    .or_else(|| existing_state.map(|state| state.last_sync_at))
                    .unwrap_or_else(unix_timestamp)
            };

            let mut state = PathState {
                path: entry.path.clone(),
                is_dir: entry.is_dir,
                state: PathSyncState::OnlineOnly,
                size_bytes: entry.size,
                pinned,
                hydrated,
                dirty,
                error,
                last_sync_at: modified_unix,
                base_revision: revision_for_entry(entry),
                conflict_reason,
            };
            state.state = derive_path_state(&state);
            states.insert(state.path.clone(), state);
        }

        for (path, state) in existing {
            if states.contains_key(&path) {
                continue;
            }
            if state.dirty
                || state.hydrated
                || state.state == PathSyncState::Conflict
                || (state.is_dir && should_preserve_dir_state(&state))
            {
                states.insert(path, state);
            }
        }

        apply_directory_states(&mut states);
        Ok(states.into_values().collect())
    }

    async fn refresh_rclone_version(&self) {
        let config = self.current_config().await;
        let version = resolve_rclone_binary(config.rclone_bin.as_deref())
            .and_then(read_rclone_version)
            .unwrap_or_default();
        let mut runtime = self.runtime.write().await;
        runtime.rclone_version = version;
    }

    fn emit_event(&self, event: BackendEvent) {
        let _ = self.event_tx.send(event);
    }

    fn spawn_log_reader<T>(self: &Arc<Self>, reader: T, label: &'static str)
    where
        T: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let backend = self.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => backend.append_log(format!("{label}: {line}")),
                    Ok(None) => break,
                    Err(error) => {
                        backend
                            .append_log(format!("{label}: unable to read process output: {error}"));
                        break;
                    }
                }
            }
        });
    }

    async fn set_remote_configured(&self, remote_configured: bool) {
        {
            let mut runtime = self.runtime.write().await;
            runtime.remote_configured = remote_configured;
            runtime.connection_state = if remote_configured {
                ConnectionState::Ready
            } else {
                ConnectionState::Disconnected
            };
            if !remote_configured {
                runtime.filesystem_state = FilesystemState::Stopped;
            }
            runtime.last_error.clear();
        }
        if !remote_configured {
            self.stop_rescan_loop().await;
        }
        if let Err(error) = self.persist_runtime().await {
            warn!("unable to persist runtime state: {error:#}");
        }
        self.emit_event(BackendEvent::ConnectionStateChanged);
        self.emit_event(BackendEvent::FilesystemStateChanged);
    }

    async fn complete_connect(
        self: &Arc<Self>,
        config: AppConfig,
        warning: Option<String>,
    ) -> Result<()> {
        match has_remote_config(&self.paths.rclone_config_file, &config.remote_name)? {
            true => {
                if let Some(warning) = warning {
                    self.append_log(format!(
                        "{warning}; the app-owned remote was written and will be reused"
                    ));
                }
                self.set_remote_configured(true).await;
                self.emit_event(BackendEvent::AuthFlowCompleted);
                if config.auto_start_filesystem {
                    self.start_filesystem().await?;
                } else if !self.runtime.read().await.sync_paused {
                    self.spawn_rescan("connect");
                    self.restart_rescan_loop().await;
                }
                Ok(())
            }
            false => match warning {
                Some(message) => Err(anyhow!(message)),
                None => Err(anyhow!(
                    "rclone finished without writing the app-owned remote"
                )),
            },
        }
    }

    async fn reconcile_remote_state_from_disk(&self) -> Result<()> {
        let config = self.current_config().await;
        let remote_exists = has_remote_config(&self.paths.rclone_config_file, &config.remote_name)?;
        let runtime_remote_configured = self.runtime.read().await.remote_configured;
        if remote_exists != runtime_remote_configured {
            self.set_remote_configured(remote_exists).await;
        }
        Ok(())
    }

    async fn record_connection_error(&self, message: String) {
        {
            let mut runtime = self.runtime.write().await;
            runtime.connection_state = ConnectionState::Error;
            if runtime.filesystem_state == FilesystemState::Starting {
                runtime.filesystem_state = FilesystemState::Stopped;
            }
            runtime.last_error = message.clone();
        }
        self.append_log(message.clone());
        if let Err(error) = self.persist_runtime().await {
            warn!("unable to persist runtime state: {error:#}");
        }
        self.emit_event(BackendEvent::ConnectionStateChanged);
        self.emit_event(BackendEvent::FilesystemStateChanged);
        self.emit_event(BackendEvent::ErrorRaised(message));
    }

    async fn record_error(&self, message: String) {
        {
            let mut runtime = self.runtime.write().await;
            runtime.filesystem_state = FilesystemState::Error;
            runtime.last_error = message.clone();
        }
        self.append_log(message.clone());
        if let Err(error) = self.persist_runtime().await {
            warn!("unable to persist runtime state: {error:#}");
        }
        self.emit_event(BackendEvent::FilesystemStateChanged);
        self.emit_event(BackendEvent::ConnectionStateChanged);
        self.emit_event(BackendEvent::ErrorRaised(message));
    }

    async fn fail_filesystem_start(&self, message: String) -> Result<()> {
        self.filesystem_session
            .lock()
            .expect("filesystem session poisoned")
            .take();
        self.underlay_root
            .lock()
            .expect("underlay root poisoned")
            .take();

        {
            let mut runtime = self.runtime.write().await;
            apply_filesystem_start_failure(&mut runtime, message.clone());
        }
        self.persist_runtime().await?;
        self.append_log(message.clone());
        self.emit_event(BackendEvent::FilesystemStateChanged);
        self.emit_event(BackendEvent::ConnectionStateChanged);
        self.emit_event(BackendEvent::ErrorRaised(message));
        Ok(())
    }

    fn append_log(&self, line: String) {
        let stamped_line = format!("{} {}", log_timestamp(), line);
        {
            let mut logs = self.recent_logs.lock().expect("logs poisoned");
            if logs.len() == MAX_RECENT_LOGS {
                logs.pop_front();
            }
            logs.push_back(stamped_line.clone());
        }
        {
            let mut runtime = self.runtime_write_guard();
            runtime.last_log_line = stamped_line;
        }
        if let Err(error) = self.persist_runtime_blocking() {
            warn!("unable to persist runtime state: {error:#}");
        }
        self.emit_event(BackendEvent::LogsUpdated);
    }

    async fn persist_runtime(&self) -> Result<()> {
        let runtime = self.runtime.read().await;
        self.state_store.save(&RuntimeState {
            remote_configured: runtime.remote_configured,
            connection_state: runtime.connection_state,
            filesystem_state: runtime.filesystem_state,
            sync_state: runtime.sync_state,
            last_error: runtime.last_error.clone(),
            last_sync_error: runtime.last_sync_error.clone(),
            last_log_line: runtime.last_log_line.clone(),
            pinned_relative_paths: runtime.pinned_relative_paths.iter().cloned().collect(),
            pending_downloads: runtime.pending_downloads,
            pending_uploads: runtime.pending_uploads,
            conflict_relative_paths: runtime.conflict_relative_paths.iter().cloned().collect(),
            last_sync_at: runtime.last_sync_at,
            sync_paused: runtime.sync_paused,
        })
    }

    async fn stop_connect_process(&self) -> Result<()> {
        {
            let mut generation = self.connect_generation.lock().await;
            *generation += 1;
        }
        if let Some(mut child) = self.connect_child.lock().await.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        Ok(())
    }

    async fn stop_rescan_loop(&self) {
        let mut generation = self.rescan_generation.lock().await;
        *generation += 1;
    }

    async fn restart_rescan_loop(self: &Arc<Self>) {
        let generation = {
            let mut generation = self.rescan_generation.lock().await;
            *generation += 1;
            *generation
        };
        let runtime = self.runtime.read().await;
        if !runtime.remote_configured || runtime.sync_paused {
            return;
        }
        drop(runtime);

        let backend = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(RESCAN_INTERVAL).await;
                let current_generation = *backend.rescan_generation.lock().await;
                if current_generation != generation {
                    return;
                }
                let runtime = backend.runtime.read().await;
                if !runtime.remote_configured || runtime.sync_paused {
                    return;
                }
                drop(runtime);
                if let Err(error) = backend.rescan().await {
                    backend.append_log(format!("periodic remote rescan failed: {error}"));
                }
            }
        });
    }

    fn spawn_rescan(self: &Arc<Self>, context: &'static str) {
        let backend = self.clone();
        tokio::spawn(async move {
            let runtime = backend.runtime.read().await;
            if !runtime.remote_configured || runtime.sync_paused {
                return;
            }
            drop(runtime);
            if let Err(error) = backend.rescan().await {
                backend.append_log(format!("{context} remote rescan failed: {error}"));
            }
        });
    }

    fn enqueue_upload(self: &Arc<Self>, relative_path: String, allow_while_paused: bool) {
        if self.runtime_read_guard().sync_paused && !allow_while_paused {
            self.append_log(format!(
                "deferred upload for {relative_path} while sync is paused"
            ));
            return;
        }
        {
            let mut runtime = self.runtime_write_guard();
            runtime.pending_uploads = runtime.pending_uploads.saturating_add(1);
            runtime.sync_state = SyncState::Syncing;
        }
        if let Err(error) = self.persist_runtime_blocking() {
            warn!("unable to persist runtime state: {error:#}");
        }
        self.emit_event(BackendEvent::SyncStateChanged);

        let backend = self.clone();
        self.runtime_handle.spawn(async move {
            let outcome = tokio::task::spawn_blocking({
                let backend = backend.clone();
                let relative_path = relative_path.clone();
                move || backend.upload_relative_path_sync(&relative_path)
            })
            .await;

            match outcome {
                Ok(Ok(())) => {
                    backend.append_log(format!("uploaded {relative_path}"));
                    let _ = backend.rescan().await;
                }
                Ok(Err(error)) => {
                    backend.set_path_error_sync(&relative_path, error.to_string());
                }
                Err(error) => {
                    backend.set_path_error_sync(&relative_path, error.to_string());
                }
            }

            {
                let mut runtime = backend.runtime.write().await;
                runtime.pending_uploads = runtime.pending_uploads.saturating_sub(1);
                if runtime.pending_uploads == 0 && runtime.pending_downloads == 0 {
                    runtime.sync_state = if runtime.sync_paused {
                        SyncState::Paused
                    } else {
                        SyncState::Idle
                    };
                }
            }
            let _ = backend.persist_runtime().await;
            backend.emit_event(BackendEvent::SyncStateChanged);
        });
    }

    fn enqueue_dirty_uploads(self: &Arc<Self>) -> Result<u32> {
        let dirty_paths = self
            .path_state_store
            .all()?
            .into_iter()
            .filter(|state| !state.is_dir && state.dirty)
            .map(|state| state.path)
            .collect::<BTreeSet<_>>();
        for path in &dirty_paths {
            self.enqueue_upload(path.clone(), true);
        }
        Ok(dirty_paths.len() as u32)
    }

    fn set_path_error_sync(&self, relative_path: &str, message: String) {
        let mut states = self
            .path_state_store
            .get_many(&[relative_path.to_string()])
            .unwrap_or_default();
        let mut state = states.pop().unwrap_or_else(|| PathState {
            path: relative_path.to_string(),
            is_dir: false,
            state: PathSyncState::Error,
            size_bytes: 0,
            pinned: false,
            hydrated: false,
            dirty: false,
            error: String::new(),
            last_sync_at: unix_timestamp(),
            base_revision: String::new(),
            conflict_reason: String::new(),
        });
        if message.contains("conflict") {
            state.state = PathSyncState::Conflict;
            state.conflict_reason = message.clone();
        } else {
            state.state = PathSyncState::Error;
            state.error = message.clone();
        }
        state.dirty = true;
        state.last_sync_at = unix_timestamp();
        let _ = self.path_state_store.upsert_many(&[state]);
        let _ = self.rebuild_path_state_snapshot_sync();
        self.append_log(message.clone());
        self.emit_path_state_refresh(&[relative_path.to_string()]);
    }

    fn set_path_download_error_sync(&self, relative_path: &str, message: String) {
        let mut states = self
            .path_state_store
            .get_many(&[relative_path.to_string()])
            .unwrap_or_default();
        let mut state = states.pop().unwrap_or_else(|| PathState {
            path: relative_path.to_string(),
            is_dir: false,
            state: PathSyncState::Error,
            size_bytes: 0,
            pinned: false,
            hydrated: false,
            dirty: false,
            error: String::new(),
            last_sync_at: unix_timestamp(),
            base_revision: String::new(),
            conflict_reason: String::new(),
        });
        state.hydrated = false;
        state.error = message.clone();
        state.conflict_reason.clear();
        state.dirty = false;
        state.state = PathSyncState::Error;
        state.last_sync_at = unix_timestamp();
        let _ = self.path_state_store.upsert_many(&[state]);
        let _ = self.rebuild_path_state_snapshot_sync();
        self.append_log(message.clone());
        self.emit_path_state_refresh(&[relative_path.to_string()]);
    }

    fn hydrate_relative_path_sync(&self, relative_path: &str) -> Result<PathBuf> {
        let current = self
            .path_state_store
            .get_many(&[relative_path.to_string()])?
            .into_iter()
            .next()
            .with_context(|| format!("unknown path {}", relative_path))?;
        if current.is_dir {
            let path = self.backing_file_path(relative_path)?;
            fs::create_dir_all(&path)
                .with_context(|| format!("unable to create {}", path.display()))?;
            return Ok(path);
        }

        let local_path = self.ensure_backing_parent(relative_path)?;
        if local_path.exists() {
            return Ok(local_path);
        }

        {
            let mut runtime = self.runtime_write_guard();
            runtime.pending_downloads = runtime.pending_downloads.saturating_add(1);
            runtime.sync_state = SyncState::Syncing;
        }
        let _ = self.persist_runtime_blocking();
        self.emit_event(BackendEvent::SyncStateChanged);
        let result = (|| -> Result<PathBuf> {
            let config = self.config_read_guard().clone();
            let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
            let status = std::process::Command::new(binary)
                .args(build_download_args(
                    &config,
                    &self.paths,
                    relative_path,
                    &local_path,
                ))
                .status()
                .context("failed to execute rclone copyto")?;
            if !status.success() {
                bail!("rclone copyto failed for {relative_path}");
            }

            let mut state = current;
            state.hydrated = true;
            state.error.clear();
            state.state = derive_path_state(&state);
            state.last_sync_at = unix_timestamp();
            state.size_bytes = fs::metadata(&local_path)
                .map(|metadata| metadata.len())
                .unwrap_or(state.size_bytes);
            self.path_state_store.upsert_many(&[state])?;
            Ok(local_path.clone())
        })();
        {
            let mut runtime = self.runtime_write_guard();
            runtime.pending_downloads = runtime.pending_downloads.saturating_sub(1);
            if runtime.pending_downloads == 0 && runtime.pending_uploads == 0 {
                runtime.sync_state = if runtime.sync_paused {
                    SyncState::Paused
                } else {
                    SyncState::Idle
                };
            }
        }
        let _ = self.persist_runtime_blocking();
        self.emit_event(BackendEvent::SyncStateChanged);

        if let Err(error) = &result {
            let _ = remove_file_if_exists(&local_path);
            self.set_path_download_error_sync(relative_path, error.to_string());
        }

        result
    }

    fn evict_relative_path_sync(&self, relative_path: &str) -> Result<()> {
        let mut state = self
            .path_state_store
            .get_many(&[relative_path.to_string()])?
            .into_iter()
            .next()
            .with_context(|| format!("unknown path {}", relative_path))?;
        if state.dirty || state.state == PathSyncState::Conflict {
            bail!("cannot evict {} while it has local changes", relative_path);
        }
        let path = self.backing_file_path(relative_path)?;
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("unable to remove {}", path.display()))?;
            remove_empty_parent_dirs(&path, &self.backing_root_access_path()?)?;
        }
        state.hydrated = false;
        state.pinned = false;
        state.state = derive_path_state(&state);
        state.last_sync_at = unix_timestamp();
        self.path_state_store.upsert_many(&[state])?;
        Ok(())
    }

    fn upload_relative_path_sync(&self, relative_path: &str) -> Result<()> {
        let mut state = self
            .path_state_store
            .get_many(&[relative_path.to_string()])?
            .into_iter()
            .next()
            .with_context(|| format!("unknown path {}", relative_path))?;
        let local_path = self.backing_file_path(relative_path)?;
        if !local_path.exists() {
            bail!("local backing file is missing for {}", relative_path);
        }

        if !state.base_revision.is_empty() {
            let config = self.config_read_guard().clone();
            let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
            let output = std::process::Command::new(binary)
                .args(build_lsjson_single_args(
                    &config,
                    &self.paths,
                    relative_path,
                ))
                .output()
                .context("failed to execute rclone lsjson for conflict detection")?;
            if output.status.success() {
                let payload = String::from_utf8_lossy(&output.stdout).to_string();
                let remote =
                    serde_json::from_str::<Vec<RcloneListEntry>>(&payload).unwrap_or_default();
                if let Some(remote_entry) =
                    remote.into_iter().find(|entry| entry.path == relative_path)
                {
                    let revision = revision_for_entry(&remote_entry);
                    if revision != state.base_revision {
                        bail!("conflict detected for {}", relative_path);
                    }
                }
            }
        }

        let config = self.config_read_guard().clone();
        let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
        let status = std::process::Command::new(binary)
            .args(build_upload_args(
                &config,
                &self.paths,
                relative_path,
                &local_path,
            ))
            .status()
            .context("failed to execute rclone copyto")?;
        if !status.success() {
            bail!("rclone copyto upload failed for {}", relative_path);
        }

        state.dirty = false;
        state.error.clear();
        state.conflict_reason.clear();
        state.hydrated = true;
        state.size_bytes = fs::metadata(&local_path)
            .map(|metadata| metadata.len())
            .unwrap_or(state.size_bytes);
        state.last_sync_at = unix_timestamp();
        state.base_revision = format!("local-{}", state.last_sync_at);
        state.state = derive_path_state(&state);
        self.path_state_store.upsert_many(&[state])?;
        Ok(())
    }

    fn create_local_entry_sync(&self, relative_path: &str, is_dir: bool) -> Result<()> {
        let target = self.ensure_backing_parent(relative_path)?;
        if is_dir {
            fs::create_dir_all(&target)
                .with_context(|| format!("unable to create {}", target.display()))?;
        } else {
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(&target)
                .with_context(|| format!("unable to create {}", target.display()))?;
        }
        let state = PathState {
            path: relative_path.to_string(),
            is_dir,
            state: if is_dir {
                PathSyncState::AvailableLocal
            } else {
                PathSyncState::Syncing
            },
            size_bytes: if is_dir {
                0
            } else {
                fs::metadata(&target).map(|m| m.len()).unwrap_or(0)
            },
            pinned: false,
            hydrated: !is_dir,
            dirty: !is_dir,
            error: String::new(),
            last_sync_at: unix_timestamp(),
            base_revision: String::new(),
            conflict_reason: String::new(),
        };
        self.path_state_store.upsert_many(&[state])?;
        Ok(())
    }

    fn create_remote_dir_sync(&self, relative_path: &str) -> Result<()> {
        let config = self.config_read_guard().clone();
        let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
        let status = std::process::Command::new(binary)
            .args(build_mkdir_args(&config, &self.paths, relative_path))
            .status()
            .context("failed to execute rclone mkdir")?;
        if !status.success() {
            bail!("rclone mkdir failed for {}", relative_path);
        }

        let state = PathState {
            path: relative_path.to_string(),
            is_dir: true,
            state: PathSyncState::OnlineOnly,
            size_bytes: 0,
            pinned: false,
            hydrated: false,
            dirty: false,
            error: String::new(),
            last_sync_at: unix_timestamp(),
            base_revision: format!("dir-{}", unix_timestamp()),
            conflict_reason: String::new(),
        };
        self.path_state_store.upsert_many(&[state])?;
        Ok(())
    }

    fn remove_path_sync(&self, relative_path: &str, is_dir: bool) -> Result<()> {
        let state = self
            .path_state_store
            .get_many(&[relative_path.to_string()])?
            .into_iter()
            .next();
        let remote_backed = state
            .as_ref()
            .is_some_and(|existing| !existing.base_revision.is_empty());

        if remote_backed {
            let config = self.config_read_guard().clone();
            let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
            let args = if is_dir {
                build_rmdir_args(&config, &self.paths, relative_path)
            } else {
                build_deletefile_args(&config, &self.paths, relative_path)
            };
            let status = std::process::Command::new(binary)
                .args(args)
                .status()
                .context("failed to execute rclone delete")?;
            if !status.success() {
                bail!("rclone delete failed for {}", relative_path);
            }
        }

        let local_path = self.backing_file_path(relative_path)?;
        let local_cleanup_error = if remote_backed {
            match remove_local_backing_path(&local_path, is_dir) {
                Ok(()) => None,
                Err(error) if should_ignore_missing_local_path(&error) => None,
                Err(error) => Some(error),
            }
        } else {
            remove_local_backing_path(&local_path, is_dir)?;
            None
        };

        let remaining = self
            .path_state_store
            .all()?
            .into_iter()
            .filter(|state| {
                state.path != relative_path && !state.path.starts_with(&format!("{relative_path}/"))
            })
            .collect::<Vec<_>>();
        self.path_state_store.replace_all(&remaining)?;
        if let Some(error) = local_cleanup_error {
            self.append_log(format!(
                "remote delete succeeded for {relative_path}, but local cleanup needs attention: {error:#}"
            ));
        }
        Ok(())
    }

    fn rename_path_sync(&self, from: &str, to: &str) -> Result<()> {
        let state = self
            .path_state_store
            .get_many(&[from.to_string()])?
            .into_iter()
            .next()
            .with_context(|| format!("unknown path {}", from))?;
        let remote_backed = !state.base_revision.is_empty();

        if remote_backed {
            let config = self.config_read_guard().clone();
            let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
            let status = std::process::Command::new(binary)
                .args(build_moveto_args(&config, &self.paths, from, to))
                .status()
                .context("failed to execute rclone moveto")?;
            if !status.success() {
                bail!("rclone moveto failed from {} to {}", from, to);
            }
        }

        let source_local = self.backing_file_path(from)?;
        let target_local = self.ensure_backing_parent(to)?;
        let local_rename_error = if remote_backed {
            match rename_local_backing_path(&source_local, &target_local) {
                Ok(()) => None,
                Err(error) if should_ignore_missing_local_path(&error) => None,
                Err(error) => Some(error),
            }
        } else {
            rename_local_backing_path(&source_local, &target_local)?;
            None
        };

        let mut all = self.path_state_store.all()?;
        for item in &mut all {
            if item.path == from {
                item.path = to.to_string();
            } else if item.path.starts_with(&format!("{from}/")) {
                item.path = format!("{to}/{}", item.path.trim_start_matches(&format!("{from}/")));
            }
        }
        self.path_state_store.replace_all(&all)?;
        if let Some(error) = local_rename_error {
            self.append_log(format!(
                "remote rename succeeded from {from} to {to}, but local cleanup needs attention: {error:#}"
            ));
        }
        Ok(())
    }

    fn clear_backing_root(&self) -> Result<()> {
        let config = self.config_read_guard().clone();
        let backing_path = config.backing_dir_path();
        if backing_path.exists() {
            fs::remove_dir_all(&backing_path)
                .with_context(|| format!("unable to remove {}", backing_path.display()))?;
        }
        Ok(())
    }
}

struct FuseBridge {
    backend: std::sync::Weak<RcloneBackend>,
}

impl FuseBridge {
    fn backend(&self) -> io::Result<Arc<RcloneBackend>> {
        self.backend
            .upgrade()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))
    }
}

impl Provider for FuseBridge {
    fn snapshot_entries(&self) -> io::Result<Vec<VirtualEntry>> {
        let backend = self.backend()?;
        let states = backend.path_state_store.all().map_err(io_error)?;
        Ok(states
            .into_iter()
            .map(|state| VirtualEntry {
                path: state.path,
                is_dir: state.is_dir,
                size_bytes: state.size_bytes,
                modified_unix: state.last_sync_at,
            })
            .collect())
    }

    fn open_file(&self, path: &str, request: OpenRequest) -> io::Result<File> {
        let backend = self.backend()?;
        if request.create {
            backend
                .create_local_entry_sync(path, false)
                .map_err(io_error)?;
        } else {
            backend.hydrate_relative_path_sync(path).map_err(io_error)?;
        }
        backend
            .rebuild_path_state_snapshot_sync()
            .map_err(io_error)?;
        backend.emit_path_state_refresh(&[path.to_string()]);
        let target = backend.ensure_backing_parent(path).map_err(io_error)?;
        OpenOptions::new()
            .read(true)
            .write(request.write)
            .create(request.create)
            .truncate(request.truncate)
            .open(&target)
    }

    fn create_dir(&self, path: &str) -> io::Result<()> {
        let backend = self.backend()?;
        backend.create_remote_dir_sync(path).map_err(io_error)?;
        backend
            .rebuild_path_state_snapshot_sync()
            .map_err(io_error)?;
        backend.emit_path_state_refresh(&[path.to_string()]);
        Ok(())
    }

    fn remove_file(&self, path: &str) -> io::Result<()> {
        let backend = self.backend()?;
        backend.remove_path_sync(path, false).map_err(io_error)?;
        backend
            .rebuild_path_state_snapshot_sync()
            .map_err(io_error)?;
        backend.emit_path_state_refresh(&[path.to_string()]);
        Ok(())
    }

    fn remove_dir(&self, path: &str) -> io::Result<()> {
        let backend = self.backend()?;
        backend.remove_path_sync(path, true).map_err(io_error)?;
        backend
            .rebuild_path_state_snapshot_sync()
            .map_err(io_error)?;
        backend.emit_path_state_refresh(&[path.to_string()]);
        Ok(())
    }

    fn rename_path(&self, from: &str, to: &str) -> io::Result<()> {
        let backend = self.backend()?;
        backend.rename_path_sync(from, to).map_err(io_error)?;
        backend
            .rebuild_path_state_snapshot_sync()
            .map_err(io_error)?;
        backend.emit_path_state_refresh(&[from.to_string(), to.to_string()]);
        Ok(())
    }

    fn set_len(&self, path: &str, size: u64) -> io::Result<()> {
        let backend = self.backend()?;
        let target = backend.backing_file_path(path).map_err(io_error)?;
        let file = OpenOptions::new().write(true).open(&target)?;
        file.set_len(size)?;
        let mut state = backend
            .path_state_store
            .get_many(&[path.to_string()])
            .map_err(io_error)?
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        state.size_bytes = size;
        state.dirty = true;
        state.hydrated = true;
        state.state = PathSyncState::Syncing;
        state.last_sync_at = unix_timestamp();
        backend
            .path_state_store
            .upsert_many(&[state])
            .map_err(io_error)?;
        backend
            .rebuild_path_state_snapshot_sync()
            .map_err(io_error)?;
        backend.emit_path_state_refresh(&[path.to_string()]);
        backend.enqueue_upload(path.to_string(), false);
        Ok(())
    }

    fn finish_write(&self, path: &str) -> io::Result<()> {
        let backend = self.backend()?;
        let mut state = backend
            .path_state_store
            .get_many(&[path.to_string()])
            .map_err(io_error)?
            .into_iter()
            .next()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        state.dirty = true;
        state.hydrated = true;
        state.state = PathSyncState::Syncing;
        state.last_sync_at = unix_timestamp();
        backend
            .path_state_store
            .upsert_many(&[state])
            .map_err(io_error)?;
        backend
            .rebuild_path_state_snapshot_sync()
            .map_err(io_error)?;
        backend.emit_path_state_refresh(&[path.to_string()]);
        backend.enqueue_upload(path.to_string(), false);
        Ok(())
    }
}

fn has_remote_config(config_file: &Path, remote_name: &str) -> Result<bool> {
    Ok(read_remote_config_section(config_file, remote_name)?.is_some())
}

fn read_remote_config_section(
    config_file: &Path,
    remote_name: &str,
) -> Result<Option<RemoteConfigSection>> {
    if !config_file.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(config_file)
        .with_context(|| format!("unable to read {}", config_file.display()))?;
    let marker = format!("[{remote_name}]");
    let mut found_section = false;
    let mut in_target_section = false;
    let mut options = BTreeMap::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_target_section {
                break;
            }
            in_target_section = trimmed == marker;
            found_section |= in_target_section;
            continue;
        }

        if !in_target_section
            || trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with(';')
        {
            continue;
        }

        if let Some((key, value)) = trimmed.split_once('=') {
            options.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    if !found_section {
        return Ok(None);
    }

    Ok(Some(RemoteConfigSection { options }))
}

fn remote_config_needs_repair(config_file: &Path, remote_name: &str) -> Result<bool> {
    Ok(read_remote_config_section(config_file, remote_name)?
        .is_some_and(|section| section.is_onedrive() && section.missing_drive_metadata()))
}

fn migrate_legacy_onedrive_remote(
    config_file: &Path,
    remote_name: &str,
    rclone_bin_override: Option<&Path>,
) -> Result<bool> {
    let Some(section) = read_remote_config_section(config_file, remote_name)? else {
        return Ok(false);
    };
    if !section.is_onedrive() || !section.missing_drive_metadata() {
        return Ok(false);
    }

    let result = (|| -> Result<()> {
        let binary = resolve_rclone_binary(rclone_bin_override)?;
        let access_token = refresh_onedrive_access_token(&section, &binary)?;
        let drive = read_onedrive_drive_metadata(&section, &access_token)?;
        update_onedrive_drive_metadata(config_file, remote_name, &binary, &drive)?;
        Ok(())
    })();

    match result {
        Ok(()) => Ok(true),
        Err(error) => Err(anyhow!(build_remote_repair_message(&error.to_string()))),
    }
}

fn refresh_onedrive_access_token(section: &RemoteConfigSection, binary: &Path) -> Result<String> {
    let token = section
        .option("token")
        .context("the app-owned rclone profile is missing its OAuth token")?;
    let token_json =
        serde_json::from_str::<Value>(token).context("unable to parse the stored OAuth token")?;
    let refresh_token = token_json
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .context("the stored OAuth token does not include a refresh_token")?;
    let client_id = section
        .option("client_id")
        .unwrap_or(RCLONE_ONEDRIVE_DEFAULT_CLIENT_ID);
    let client_secret = match (section.option("client_id"), section.option("client_secret")) {
        (_, Some(secret)) => Some(reveal_rclone_secret_or_raw(binary, secret)),
        (Some(_), None) => None,
        (None, None) => Some(reveal_rclone_secret(
            binary,
            RCLONE_ONEDRIVE_DEFAULT_CLIENT_SECRET_OBSCURED,
        )?),
    };
    let token_url = onedrive_token_url(section)?;
    let request = ureq::post(&token_url);
    let response = if let Some(client_secret) = client_secret.as_deref() {
        request.send_form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
    } else {
        request.send_form(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
    }
    .context("unable to refresh the stored OneDrive token")?;
    let refreshed = response
        .into_json::<RefreshedAccessToken>()
        .context("unable to parse the refreshed OneDrive token response")?;
    Ok(refreshed.access_token)
}

fn read_onedrive_drive_metadata(
    section: &RemoteConfigSection,
    access_token: &str,
) -> Result<DriveMetadata> {
    let region = section.option("region").unwrap_or("global");
    let drive_url = format!("{}/v1.0/me/drive", onedrive_graph_base_url(region)?);
    let drive = ureq::get(&drive_url)
        .set("Authorization", &format!("Bearer {access_token}"))
        .call()
        .context("unable to read OneDrive drive metadata from Microsoft Graph")?
        .into_json::<DriveMetadata>()
        .context("unable to parse OneDrive drive metadata")?;
    if drive.drive_id.trim().is_empty() || drive.drive_type.trim().is_empty() {
        bail!("Microsoft Graph returned incomplete OneDrive drive metadata");
    }
    Ok(drive)
}

fn update_onedrive_drive_metadata(
    config_file: &Path,
    remote_name: &str,
    binary: &Path,
    drive: &DriveMetadata,
) -> Result<()> {
    let output = std::process::Command::new(binary)
        .args([
            OsString::from("config"),
            OsString::from("update"),
            OsString::from(remote_name),
            OsString::from("drive_id"),
            OsString::from(&drive.drive_id),
            OsString::from("drive_type"),
            OsString::from(&drive.drive_type),
            OsString::from("config_refresh_token=false"),
            OsString::from("--config"),
            config_file.as_os_str().to_os_string(),
            OsString::from("--non-interactive"),
        ])
        .output()
        .context("failed to update the app-owned rclone profile")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(
            "rclone config update failed{}",
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        );
    }

    Ok(())
}

fn reveal_rclone_secret_or_raw(binary: &Path, secret: &str) -> String {
    reveal_rclone_secret(binary, secret).unwrap_or_else(|_| secret.to_string())
}

fn reveal_rclone_secret(binary: &Path, secret: &str) -> Result<String> {
    let output = std::process::Command::new(binary)
        .args(["reveal", secret])
        .output()
        .context("failed to decode the stored OneDrive client secret")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(
            "rclone reveal failed{}",
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        );
    }

    Ok(String::from_utf8(output.stdout)
        .context("rclone reveal returned invalid utf-8")?
        .trim()
        .to_string())
}

fn onedrive_token_url(section: &RemoteConfigSection) -> Result<String> {
    if let Some(token_url) = section.option("token_url") {
        return Ok(token_url.to_string());
    }

    let auth_base = onedrive_login_base_url(section.option("region").unwrap_or("global"))?;
    let tenant = section.option("tenant").unwrap_or("common");
    Ok(format!("{auth_base}/{tenant}/oauth2/v2.0/token"))
}

fn onedrive_login_base_url(region: &str) -> Result<&'static str> {
    match region {
        "global" => Ok("https://login.microsoftonline.com"),
        "us" => Ok("https://login.microsoftonline.us"),
        "de" => Ok("https://login.microsoftonline.de"),
        "cn" => Ok("https://login.chinacloudapi.cn"),
        other => bail!("unsupported OneDrive region {other}"),
    }
}

fn onedrive_graph_base_url(region: &str) -> Result<&'static str> {
    match region {
        "global" => Ok("https://graph.microsoft.com"),
        "us" => Ok("https://graph.microsoft.us"),
        "de" => Ok("https://graph.microsoft.de"),
        "cn" => Ok("https://microsoftgraph.chinacloudapi.cn"),
        other => bail!("unsupported OneDrive region {other}"),
    }
}

fn build_remote_repair_message(details: &str) -> String {
    format!(
        "The app-owned OneDrive profile was created by an older release and needs repair: {details}. Use Repair Remote to rebuild only the rclone profile and keep hydrated bytes plus path state on this device."
    )
}

fn is_legacy_onedrive_remote_error(message: &str) -> bool {
    message.contains(LEGACY_ONEDRIVE_DRIVE_METADATA_ERROR)
}

fn expand_selected_paths(root_path: &Path, raw_paths: &[String]) -> Result<BTreeSet<String>> {
    let mut files = BTreeSet::new();
    for raw_path in raw_paths {
        let relative = relative_path_for(root_path, Path::new(raw_path))?;
        let absolute = root_path.join(&relative);
        collect_selected_files(root_path, &absolute, &mut files)?;
    }
    Ok(files)
}

fn expand_retry_paths(
    root_path: &Path,
    raw_paths: &[String],
    states: &[PathState],
) -> Result<BTreeSet<String>> {
    let state_map = states
        .iter()
        .map(|state| (state.path.clone(), state))
        .collect::<HashMap<_, _>>();
    let mut retryable = BTreeSet::new();

    for raw_path in raw_paths {
        let relative = relative_string(&relative_path_for(root_path, Path::new(raw_path))?);
        let Some(state) = state_map.get(&relative) else {
            bail!("unknown path {relative}");
        };
        if !state.is_dir {
            retryable.insert(relative);
            continue;
        }

        let prefix = format!("{}/", state.path);
        retryable.extend(
            states
                .iter()
                .filter(|candidate| {
                    !candidate.is_dir
                        && candidate.path.starts_with(&prefix)
                        && is_retryable_state(candidate)
                })
                .map(|candidate| candidate.path.clone()),
        );
    }

    Ok(retryable)
}

fn is_retryable_state(state: &PathState) -> bool {
    state.dirty
        || state.state == PathSyncState::Error
        || state.state == PathSyncState::Conflict
        || !state.error.is_empty()
        || !state.conflict_reason.is_empty()
}

fn collect_selected_files(
    root_path: &Path,
    path: &Path,
    files: &mut BTreeSet<String>,
) -> Result<()> {
    let metadata =
        fs::metadata(path).with_context(|| format!("unable to inspect {}", path.display()))?;
    if metadata.is_dir() {
        for entry in
            fs::read_dir(path).with_context(|| format!("unable to read {}", path.display()))?
        {
            let entry = entry.with_context(|| format!("unable to read {}", path.display()))?;
            collect_selected_files(root_path, &entry.path(), files)?;
        }
        return Ok(());
    }

    if metadata.is_file() {
        let relative = relative_path_for(root_path, path)?;
        files.insert(relative_string(&relative));
    }
    Ok(())
}

fn relative_path_for(root_path: &Path, raw_path: &Path) -> Result<PathBuf> {
    let absolute = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        root_path.join(raw_path)
    };
    let relative = absolute
        .strip_prefix(root_path)
        .with_context(|| format!("{} is outside the OneDrive root path", absolute.display()))?;
    if relative.as_os_str().is_empty() {
        bail!("select a file or directory inside the OneDrive folder");
    }

    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(value) => normalized.push(value),
            std::path::Component::CurDir => {}
            _ => bail!("unsupported path outside the OneDrive folder"),
        }
    }
    if normalized.as_os_str().is_empty() {
        bail!("select a file or directory inside the OneDrive folder");
    }
    Ok(normalized)
}

fn relative_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn parse_rclone_mod_time_unix(raw: &str) -> Option<u64> {
    if raw.trim().is_empty() {
        return None;
    }

    OffsetDateTime::parse(raw, &Rfc3339)
        .ok()
        .and_then(|value| u64::try_from(value.unix_timestamp()).ok())
}

fn remove_empty_parent_dirs(path: &Path, stop_at: &Path) -> Result<()> {
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir == stop_at {
            break;
        }
        match fs::read_dir(dir) {
            Ok(entries) => {
                let mut entries = entries;
                if entries.next().is_none() {
                    fs::remove_dir(dir)
                        .with_context(|| format!("unable to remove {}", dir.display()))?;
                    current = dir.parent();
                    continue;
                }
                break;
            }
            Err(error) => {
                return Err(error).with_context(|| format!("unable to inspect {}", dir.display()));
            }
        }
    }
    Ok(())
}

fn remove_local_backing_path(path: &Path, is_dir: bool) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    if is_dir {
        fs::remove_dir_all(path).with_context(|| format!("unable to remove {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("unable to remove {}", path.display()))?;
    }
    Ok(())
}

fn migrate_backing_root(
    previous_root: &Path,
    next_root: &Path,
    backing_dir_name: &str,
) -> Result<()> {
    if previous_root == next_root {
        return Ok(());
    }

    let source = previous_root.join(backing_dir_name);
    if !source.exists() {
        return Ok(());
    }
    if !source.is_dir() {
        bail!(
            "local backing path is not a directory: {}",
            source.display()
        );
    }

    let destination = next_root.join(backing_dir_name);
    if destination.exists() {
        if !destination.is_dir() {
            bail!(
                "target backing path is not a directory: {}",
                destination.display()
            );
        }
        if !directory_is_empty(&destination)? {
            bail!(
                "target root already contains hydrated bytes in {}",
                destination.display()
            );
        }
        fs::remove_dir_all(&destination)
            .with_context(|| format!("unable to clear {}", destination.display()))?;
    }

    move_dir_all(&source, &destination)
}

fn move_dir_all(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(libc::EXDEV) => {
            copy_dir_all(source, destination)?;
            fs::remove_dir_all(source)
                .with_context(|| format!("unable to remove {}", source.display()))?;
            Ok(())
        }
        Err(error) => Err(error).with_context(|| {
            format!(
                "unable to move {} to {}",
                source.display(),
                destination.display()
            )
        }),
    }
}

fn copy_dir_all(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("unable to create {}", destination.display()))?;

    for entry in
        fs::read_dir(source).with_context(|| format!("unable to inspect {}", source.display()))?
    {
        let entry = entry.with_context(|| format!("unable to read {}", source.display()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = entry
            .metadata()
            .with_context(|| format!("unable to stat {}", source_path.display()))?;
        if metadata.is_dir() {
            copy_dir_all(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "unable to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn directory_is_empty(path: &Path) -> Result<bool> {
    let mut entries =
        fs::read_dir(path).with_context(|| format!("unable to inspect {}", path.display()))?;
    Ok(entries.next().is_none())
}

fn rename_local_backing_path(source: &Path, target: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }

    fs::rename(source, target).with_context(|| {
        format!(
            "unable to rename {} to {}",
            source.display(),
            target.display()
        )
    })
}

fn should_ignore_missing_local_path(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<io::Error>()
        .is_some_and(|inner| inner.kind() == io::ErrorKind::NotFound)
}

#[derive(Debug, Clone, Deserialize)]
struct RcloneListEntry {
    #[serde(rename = "Path", default)]
    path: String,
    #[serde(rename = "IsDir", default)]
    is_dir: bool,
    #[serde(rename = "Size", default)]
    size: u64,
    #[serde(rename = "ModTime", default)]
    mod_time: String,
    #[serde(rename = "Hashes", default)]
    hashes: BTreeMap<String, String>,
}

fn revision_for_entry(entry: &RcloneListEntry) -> String {
    let hash_fragment = entry
        .hashes
        .iter()
        .find(|(_, value)| !value.is_empty())
        .map(|(name, value)| format!(":{name}={value}"))
        .unwrap_or_default();
    format!("{}:{}{}", entry.size, entry.mod_time, hash_fragment)
}

fn apply_filesystem_start_failure(runtime: &mut Runtime, message: String) {
    runtime.filesystem_state = FilesystemState::Error;
    runtime.connection_state = if runtime.remote_configured {
        ConnectionState::Ready
    } else {
        ConnectionState::Disconnected
    };
    runtime.last_error = message;
}

fn derive_path_state(state: &PathState) -> PathSyncState {
    if !state.conflict_reason.is_empty() {
        PathSyncState::Conflict
    } else if !state.error.is_empty() {
        PathSyncState::Error
    } else if state.dirty {
        PathSyncState::Syncing
    } else if state.pinned {
        PathSyncState::PinnedLocal
    } else if state.hydrated {
        PathSyncState::AvailableLocal
    } else {
        PathSyncState::OnlineOnly
    }
}

fn apply_directory_states(states: &mut BTreeMap<String, PathState>) {
    let file_states = states
        .values()
        .filter(|state| !state.is_dir)
        .cloned()
        .collect::<Vec<_>>();
    let mut summaries = BTreeMap::<String, PathState>::new();

    for file_state in file_states {
        let path = Path::new(&file_state.path);
        let mut current = path.parent();
        while let Some(parent) = current {
            if parent.as_os_str().is_empty() {
                break;
            }
            let key = relative_string(parent);
            let entry = summaries.entry(key.clone()).or_insert(PathState {
                path: key,
                is_dir: true,
                state: PathSyncState::OnlineOnly,
                size_bytes: 0,
                pinned: false,
                hydrated: false,
                dirty: false,
                error: String::new(),
                last_sync_at: file_state.last_sync_at,
                base_revision: String::new(),
                conflict_reason: String::new(),
            });
            entry.state = dominant_path_state(entry.state.clone(), file_state.state.clone());
            entry.pinned |= file_state.pinned;
            entry.hydrated |= file_state.hydrated;
            entry.dirty |= file_state.dirty;
            if entry.conflict_reason.is_empty() {
                entry.conflict_reason = file_state.conflict_reason.clone();
            }
            if entry.error.is_empty() {
                entry.error = file_state.error.clone();
            }
            current = parent.parent();
        }
    }

    for (path, summary) in summaries {
        states
            .entry(path)
            .and_modify(|state| {
                state.is_dir = true;
                state.state = summary.state.clone();
                state.pinned = summary.pinned;
                state.hydrated = summary.hydrated;
                state.dirty = summary.dirty;
                state.error = summary.error.clone();
                state.conflict_reason = summary.conflict_reason.clone();
            })
            .or_insert(summary);
    }
}

fn dominant_path_state(current: PathSyncState, next: PathSyncState) -> PathSyncState {
    fn rank(state: &PathSyncState) -> u8 {
        match state {
            PathSyncState::Error => 6,
            PathSyncState::Conflict => 5,
            PathSyncState::Syncing => 4,
            PathSyncState::PinnedLocal => 3,
            PathSyncState::AvailableLocal => 2,
            PathSyncState::OnlineOnly => 1,
        }
    }

    if rank(&next) > rank(&current) {
        next
    } else {
        current
    }
}

fn dedup_states(states: Vec<PathState>) -> Vec<PathState> {
    let mut by_path = BTreeMap::new();
    for state in states {
        by_path.insert(state.path.clone(), state);
    }
    by_path.into_values().collect()
}

fn normalize_path_state_snapshot(states: Vec<PathState>) -> Vec<PathState> {
    let mut normalized = BTreeMap::new();
    for state in states {
        if state.is_dir && !should_preserve_dir_state(&state) {
            continue;
        }
        normalized.insert(state.path.clone(), state);
    }
    apply_directory_states(&mut normalized);
    normalized.into_values().collect()
}

fn should_preserve_dir_state(state: &PathState) -> bool {
    !state.base_revision.is_empty()
        || state.pinned
        || state.hydrated
        || state.dirty
        || !state.error.is_empty()
        || !state.conflict_reason.is_empty()
}

fn affected_relative_paths(paths: &[String]) -> Vec<String> {
    let mut affected = BTreeSet::new();
    for path in paths {
        let mut current = Some(Path::new(path));
        while let Some(candidate) = current {
            if candidate.as_os_str().is_empty() {
                break;
            }
            affected.insert(relative_string(candidate));
            current = candidate.parent();
        }
    }
    affected.into_iter().collect()
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("unable to remove {}", path.display()))?;
    }
    Ok(())
}

pub fn resolve_rclone_binary(override_path: Option<&Path>) -> Result<PathBuf> {
    let path_env = env::var_os("PATH");
    resolve_rclone_binary_with_path(override_path, path_env.as_deref())
}

pub fn resolve_rclone_binary_with_path(
    override_path: Option<&Path>,
    path_env: Option<&std::ffi::OsStr>,
) -> Result<PathBuf> {
    if let Some(path) = override_path {
        if path.exists() {
            return Ok(path.to_path_buf());
        }
        bail!(
            "configured rclone binary does not exist: {}",
            path.display()
        );
    }

    let Some(path_env) = path_env else {
        bail!("rclone was not found in PATH");
    };

    for candidate_dir in env::split_paths(path_env) {
        let candidate = candidate_dir.join("rclone");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    bail!("rclone was not found; set rclone_bin or install it in PATH")
}

pub fn build_connect_args(config: &AppConfig, paths: &ProjectPaths) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("config"),
        OsString::from("create"),
        OsString::from(&config.remote_name),
        OsString::from("onedrive"),
        OsString::from("config_type"),
        OsString::from("onedrive"),
        OsString::from("region"),
        OsString::from("global"),
    ];

    if let Some(client_id) = config.custom_client_id.as_ref() {
        args.push(OsString::from("client_id"));
        args.push(OsString::from(client_id));
    }

    args.push(OsString::from("--config"));
    args.push(paths.rclone_config_file.as_os_str().to_os_string());
    args
}

pub fn build_lsjson_args(config: &AppConfig, paths: &ProjectPaths) -> Vec<OsString> {
    vec![
        OsString::from("lsjson"),
        OsString::from(format!("{}:", config.remote_name)),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
        OsString::from("--recursive"),
        OsString::from("--hash"),
        OsString::from("--no-mimetype"),
    ]
}

fn build_lsjson_single_args(
    config: &AppConfig,
    paths: &ProjectPaths,
    relative_path: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("lsjson"),
        OsString::from(format!("{}:{}", config.remote_name, relative_path)),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
        OsString::from("--files-only"),
        OsString::from("--recursive"),
        OsString::from("--hash"),
        OsString::from("--no-mimetype"),
    ]
}

fn build_download_args(
    config: &AppConfig,
    paths: &ProjectPaths,
    relative_path: &str,
    local_path: &Path,
) -> Vec<OsString> {
    vec![
        OsString::from("copyto"),
        OsString::from(format!("{}:{}", config.remote_name, relative_path)),
        local_path.as_os_str().to_os_string(),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
    ]
}

fn build_upload_args(
    config: &AppConfig,
    paths: &ProjectPaths,
    relative_path: &str,
    local_path: &Path,
) -> Vec<OsString> {
    vec![
        OsString::from("copyto"),
        local_path.as_os_str().to_os_string(),
        OsString::from(format!("{}:{}", config.remote_name, relative_path)),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
    ]
}

fn build_mkdir_args(
    config: &AppConfig,
    paths: &ProjectPaths,
    relative_path: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("mkdir"),
        OsString::from(format!("{}:{}", config.remote_name, relative_path)),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
    ]
}

fn build_deletefile_args(
    config: &AppConfig,
    paths: &ProjectPaths,
    relative_path: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("deletefile"),
        OsString::from(format!("{}:{}", config.remote_name, relative_path)),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
    ]
}

fn build_rmdir_args(
    config: &AppConfig,
    paths: &ProjectPaths,
    relative_path: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("rmdir"),
        OsString::from(format!("{}:{}", config.remote_name, relative_path)),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
    ]
}

fn build_moveto_args(
    config: &AppConfig,
    paths: &ProjectPaths,
    from: &str,
    to: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("moveto"),
        OsString::from(format!("{}:{}", config.remote_name, from)),
        OsString::from(format!("{}:{}", config.remote_name, to)),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
    ]
}

fn read_rclone_version(binary: PathBuf) -> Result<String> {
    let output = std::process::Command::new(binary)
        .arg("version")
        .output()
        .context("failed to execute rclone version")?;
    if !output.status.success() {
        return Ok(String::new());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string())
}

fn directory_size_bytes(root: &Path) -> Result<u64> {
    if !root.exists() {
        return Ok(0);
    }

    let mut total = 0_u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in
            fs::read_dir(&path).with_context(|| format!("unable to inspect {}", path.display()))?
        {
            let entry = entry.with_context(|| format!("unable to read {}", path.display()))?;
            let entry_path = entry.path();
            let metadata = entry
                .metadata()
                .with_context(|| format!("unable to stat {}", entry_path.display()))?;
            if metadata.is_dir() {
                stack.push(entry_path);
            } else if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    Ok(total)
}

fn log_timestamp() -> String {
    format!("[{}]", unix_timestamp())
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn io_error(error: anyhow::Error) -> io::Error {
    io::Error::other(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        BACKEND_NAME, RcloneBackend, RcloneListEntry, Runtime, affected_relative_paths,
        apply_filesystem_start_failure, build_connect_args, build_deletefile_args,
        build_download_args, build_lsjson_args, build_moveto_args, build_rmdir_args,
        build_upload_args, derive_path_state, expand_retry_paths, expand_selected_paths,
        is_legacy_onedrive_remote_error, normalize_path_state_snapshot, parse_rclone_mod_time_unix,
        read_remote_config_section, relative_path_for, remote_config_needs_repair,
        resolve_rclone_binary_with_path, revision_for_entry,
    };
    use openonedrive_config::{AppConfig, ProjectPaths};
    use openonedrive_ipc_types::{
        ConnectionState, FilesystemState, PathState, PathSyncState, SyncState,
    };
    use openonedrive_state::RuntimeState;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn override_binary_wins_over_path_lookup() {
        let dir = tempdir().expect("tempdir");
        let override_path = dir.path().join("custom-rclone");
        fs::write(&override_path, "#!/bin/sh\n").expect("write binary");
        let resolved = resolve_rclone_binary_with_path(Some(&override_path), Some("/bin".as_ref()))
            .expect("resolve");
        assert_eq!(resolved, override_path);
    }

    #[test]
    fn connect_args_target_current_onedrive_config_flow() {
        let dir = tempdir().expect("tempdir");
        let paths = build_paths(dir.path());
        let args = build_connect_args(&AppConfig::default(), &paths);
        let rendered = args
            .iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(
            rendered
                .windows(2)
                .any(|pair| pair == ["config_type", "onedrive"])
        );
        assert!(rendered.windows(2).any(|pair| pair == ["region", "global"]));
    }

    #[test]
    fn transfer_args_use_app_owned_config() {
        let dir = tempdir().expect("tempdir");
        let paths = build_paths(dir.path());
        let config = AppConfig::default();
        let local_path = dir.path().join("file.txt");
        let rendered = build_download_args(&config, &paths, "Docs/file.txt", &local_path)
            .into_iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(rendered.contains(&"copyto".to_string()));
        assert!(rendered.contains(&paths.rclone_config_file.display().to_string()));
    }

    #[test]
    fn transfer_and_management_args_compile() {
        let dir = tempdir().expect("tempdir");
        let paths = build_paths(dir.path());
        let config = AppConfig::default();
        let local_path = dir.path().join("file.txt");
        assert!(!build_upload_args(&config, &paths, "Docs/file.txt", &local_path).is_empty());
        let lsjson = build_lsjson_args(&config, &paths)
            .into_iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(!lsjson.is_empty());
        assert!(lsjson.contains(&"--hash".to_string()));
        assert!(!build_deletefile_args(&config, &paths, "Docs/file.txt").is_empty());
        assert!(!build_rmdir_args(&config, &paths, "Docs").is_empty());
        assert!(!build_moveto_args(&config, &paths, "Docs/a.txt", "Docs/b.txt").is_empty());
    }

    #[test]
    fn revision_tokens_prefer_hashes_when_available() {
        let mut hashes = BTreeMap::new();
        hashes.insert("QuickXorHash".into(), "abc123".into());
        let entry = RcloneListEntry {
            path: "Docs/file.txt".into(),
            is_dir: false,
            size: 42,
            mod_time: "2026-03-25T00:00:00Z".into(),
            hashes,
        };
        assert_eq!(
            revision_for_entry(&entry),
            "42:2026-03-25T00:00:00Z:QuickXorHash=abc123"
        );
    }

    #[test]
    fn parses_rclone_mod_time_as_unix_timestamp() {
        assert_eq!(
            parse_rclone_mod_time_unix("2026-03-25T00:00:00Z"),
            Some(1_774_396_800)
        );
        assert_eq!(parse_rclone_mod_time_unix(""), None);
    }

    #[test]
    fn filesystem_start_failure_sets_error_state() {
        let mut runtime = Runtime::from_state(RuntimeState::default(), true);
        runtime.filesystem_state = FilesystemState::Starting;
        runtime.connection_state = ConnectionState::Ready;

        apply_filesystem_start_failure(&mut runtime, "mount failed".into());

        assert_eq!(runtime.filesystem_state, FilesystemState::Error);
        assert_eq!(runtime.connection_state, ConnectionState::Ready);
        assert_eq!(runtime.last_error, "mount failed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn prepare_remote_repair_preserves_local_state_and_cache() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        let paths = build_paths(dir.path());

        fs::create_dir_all(&paths.rclone_config_dir).expect("config dir");
        fs::write(
            &paths.rclone_config_file,
            "[openonedrive]\ntype = onedrive\nregion = global\ntoken = {\"refresh_token\":\"refresh\"}\n",
        )
        .expect("write config");

        let cache_file = dir
            .path()
            .join("OneDrive")
            .join(".openonedrive-cache")
            .join("Docs")
            .join("keep.txt");
        fs::create_dir_all(cache_file.parent().expect("cache parent")).expect("cache dir");
        fs::write(&cache_file, "cached").expect("cache file");

        backend
            .path_state_store
            .upsert_many(&[PathState {
                path: "Docs/keep.txt".into(),
                is_dir: false,
                state: PathSyncState::AvailableLocal,
                size_bytes: 6,
                pinned: false,
                hydrated: true,
                dirty: false,
                error: String::new(),
                last_sync_at: 1,
                base_revision: "rev".into(),
                conflict_reason: String::new(),
            }])
            .expect("path state");
        {
            let mut runtime = backend.runtime.write().await;
            runtime.remote_configured = true;
            runtime.connection_state = ConnectionState::Error;
            runtime.filesystem_state = FilesystemState::Stopped;
        }
        backend.persist_runtime().await.expect("persist runtime");

        backend
            .prepare_remote_repair()
            .await
            .expect("prepare repair");

        assert!(!paths.rclone_config_file.exists());
        assert!(cache_file.exists());
        let states = backend.path_state_store.all().expect("states");
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].path, "Docs/keep.txt");

        let status = backend.status().await.expect("status");
        assert!(!status.remote_configured);
        assert!(!status.needs_remote_repair);
        assert_eq!(status.connection_state, ConnectionState::Disconnected);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn keep_local_failure_marks_sync_error_instead_of_staying_syncing() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        let requested = dir.path().join("OneDrive").join("Docs").join("missing.txt");
        fs::create_dir_all(requested.parent().expect("parent")).expect("docs dir");
        fs::write(&requested, "").expect("placeholder file");

        let error = backend
            .keep_local(&[requested.display().to_string()])
            .await
            .expect_err("keep-local should fail without path state");
        assert!(error.to_string().contains("unknown path"));

        let status = backend.status().await.expect("status");
        assert_eq!(status.sync_state, SyncState::Error);
        assert!(status.last_sync_error.contains("unknown path"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn make_online_only_failure_marks_sync_error_instead_of_staying_syncing() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        let requested = dir.path().join("OneDrive").join("Docs").join("missing.txt");
        fs::create_dir_all(requested.parent().expect("parent")).expect("docs dir");
        fs::write(&requested, "").expect("placeholder file");

        let error = backend
            .make_online_only(&[requested.display().to_string()])
            .await
            .expect_err("make-online-only should fail without path state");
        assert!(error.to_string().contains("unknown path"));

        let status = backend.status().await.expect("status");
        assert_eq!(status.sync_state, SyncState::Error);
        assert!(status.last_sync_error.contains("unknown path"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn remote_snapshot_uses_remote_mod_time_for_virtual_entries() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        let snapshot = backend
            .build_snapshot_from_remote_entries(&[RcloneListEntry {
                path: "Docs/report.pdf".into(),
                is_dir: false,
                size: 42,
                mod_time: "2026-03-25T00:00:00Z".into(),
                hashes: BTreeMap::new(),
            }])
            .expect("snapshot");

        assert_eq!(snapshot.len(), 2);
        let file_state = snapshot
            .into_iter()
            .find(|state| state.path == "Docs/report.pdf")
            .expect("file state");
        assert_eq!(file_state.last_sync_at, 1_774_396_800);
    }

    #[test]
    fn remote_config_requires_repair_without_drive_metadata() {
        let dir = tempdir().expect("tempdir");
        let paths = build_paths(dir.path());
        fs::create_dir_all(&paths.rclone_config_dir).expect("config dir");
        fs::write(
            &paths.rclone_config_file,
            "[openonedrive]\ntype = onedrive\nregion = global\ntoken = {\"refresh_token\":\"refresh\"}\n",
        )
        .expect("write config");

        let section = read_remote_config_section(&paths.rclone_config_file, "openonedrive")
            .expect("load section")
            .expect("section");
        assert!(section.is_onedrive());
        assert!(section.missing_drive_metadata());
        assert!(
            remote_config_needs_repair(&paths.rclone_config_file, "openonedrive")
                .expect("needs repair")
        );
    }

    #[test]
    fn legacy_remote_error_detection_matches_rclone_message() {
        assert!(is_legacy_onedrive_remote_error(
            "unable to get drive_id and drive_type - if you are upgrading from older versions"
        ));
        assert!(!is_legacy_onedrive_remote_error(
            "some other rclone failure"
        ));
    }

    #[test]
    fn expands_selected_directories_into_relative_files() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().join("root");
        fs::create_dir_all(root.join("docs/nested")).expect("root tree");
        fs::write(root.join("docs/readme.md"), "a").expect("write file");
        fs::write(root.join("docs/nested/spec.txt"), "b").expect("write file");

        let selected = expand_selected_paths(&root, &[root.join("docs").display().to_string()])
            .expect("expand selected paths");

        assert_eq!(
            selected.into_iter().collect::<Vec<_>>(),
            vec![
                "docs/nested/spec.txt".to_string(),
                "docs/readme.md".to_string()
            ]
        );
    }

    #[test]
    fn relative_paths_stay_inside_root() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().join("root");
        fs::create_dir_all(&root).expect("root");
        let relative = relative_path_for(&root, Path::new("Docs/readme.md")).expect("relative");
        assert_eq!(relative.display().to_string(), "Docs/readme.md");
        assert!(relative_path_for(&root, &dir.path().join("other")).is_err());
    }

    #[test]
    fn path_state_derive_prefers_conflict() {
        let state = PathState {
            path: "Docs/readme.md".into(),
            is_dir: false,
            state: PathSyncState::OnlineOnly,
            size_bytes: 1,
            pinned: false,
            hydrated: true,
            dirty: true,
            error: String::new(),
            last_sync_at: 1,
            base_revision: "rev".into(),
            conflict_reason: "remote changed".into(),
        };
        assert_eq!(derive_path_state(&state), PathSyncState::Conflict);
    }

    #[test]
    fn retry_transfer_expands_retryable_directory_files_only() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().join("root");
        let snapshot = vec![
            PathState {
                path: "Docs".into(),
                is_dir: true,
                state: PathSyncState::Conflict,
                size_bytes: 0,
                pinned: false,
                hydrated: false,
                dirty: false,
                error: String::new(),
                last_sync_at: 1,
                base_revision: "dir".into(),
                conflict_reason: String::new(),
            },
            PathState {
                path: "Docs/error.txt".into(),
                is_dir: false,
                state: PathSyncState::Error,
                size_bytes: 1,
                pinned: false,
                hydrated: true,
                dirty: false,
                error: "boom".into(),
                last_sync_at: 1,
                base_revision: "rev1".into(),
                conflict_reason: String::new(),
            },
            PathState {
                path: "Docs/clean.txt".into(),
                is_dir: false,
                state: PathSyncState::AvailableLocal,
                size_bytes: 1,
                pinned: false,
                hydrated: true,
                dirty: false,
                error: String::new(),
                last_sync_at: 1,
                base_revision: "rev2".into(),
                conflict_reason: String::new(),
            },
        ];

        let retryable = expand_retry_paths(&root, &["Docs".into()], &snapshot).expect("retry");
        assert_eq!(
            retryable.into_iter().collect::<Vec<_>>(),
            vec!["Docs/error.txt".to_string()]
        );
    }

    #[test]
    fn normalize_snapshot_rebuilds_directory_aggregates() {
        let normalized = normalize_path_state_snapshot(vec![
            PathState {
                path: "Docs".into(),
                is_dir: true,
                state: PathSyncState::PinnedLocal,
                size_bytes: 0,
                pinned: true,
                hydrated: true,
                dirty: false,
                error: String::new(),
                last_sync_at: 1,
                base_revision: String::new(),
                conflict_reason: String::new(),
            },
            PathState {
                path: "Docs/readme.md".into(),
                is_dir: false,
                state: PathSyncState::AvailableLocal,
                size_bytes: 1,
                pinned: false,
                hydrated: true,
                dirty: false,
                error: String::new(),
                last_sync_at: 2,
                base_revision: "rev".into(),
                conflict_reason: String::new(),
            },
        ]);

        assert_eq!(normalized.len(), 2);
        let docs = normalized
            .into_iter()
            .find(|state| state.path == "Docs")
            .expect("docs dir");
        assert_eq!(docs.state, PathSyncState::AvailableLocal);
        assert!(!docs.pinned);
    }

    #[test]
    fn affected_paths_include_parent_directories() {
        assert_eq!(
            affected_relative_paths(&["Docs/nested/spec.txt".into()]),
            vec![
                "Docs".to_string(),
                "Docs/nested".to_string(),
                "Docs/nested/spec.txt".to_string()
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_activity_starts_inside_async_context() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;

        backend
            .begin_sync_activity(SyncState::Scanning)
            .expect("start sync activity");
        backend
            .complete_sync_activity(None)
            .await
            .expect("complete sync activity");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn status_succeeds_inside_async_context() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;

        let status = backend.status().await.expect("status");
        assert_eq!(status.backend, BACKEND_NAME);
        assert_eq!(status.sync_state, SyncState::Idle);
        assert!(!status.needs_remote_repair);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn prepare_remote_repair_keeps_backing_bytes_and_path_state() {
        let dir = tempdir().expect("tempdir");
        let paths = build_paths(dir.path());
        fs::create_dir_all(&paths.rclone_config_dir).expect("config dir");
        fs::write(
            &paths.rclone_config_file,
            "[openonedrive]\ntype = onedrive\nregion = global\ndrive_id = drive\n\
             drive_type = personal\ntoken = {\"refresh_token\":\"refresh\"}\n",
        )
        .expect("write config");

        let mut config = AppConfig::default();
        config.root_path = dir.path().join("OneDrive");
        fs::create_dir_all(&config.root_path).expect("root dir");
        let backend = RcloneBackend::load(paths.clone(), config.clone())
            .await
            .expect("backend");

        let cached_file = config.backing_dir_path().join("Docs/readme.md");
        fs::create_dir_all(cached_file.parent().expect("parent")).expect("cache tree");
        fs::write(&cached_file, "cached").expect("cached file");
        backend
            .path_state_store
            .upsert_many(&[PathState {
                path: "Docs/readme.md".into(),
                is_dir: false,
                state: PathSyncState::AvailableLocal,
                size_bytes: 6,
                pinned: false,
                hydrated: true,
                dirty: false,
                error: String::new(),
                last_sync_at: 1,
                base_revision: "rev".into(),
                conflict_reason: String::new(),
            }])
            .expect("store path state");

        backend
            .prepare_remote_repair()
            .await
            .expect("prepare repair");

        assert!(!paths.rclone_config_file.exists());
        assert!(cached_file.exists());
        assert_eq!(
            backend.path_state_store.all().expect("path states").len(),
            1
        );

        let status = backend.status().await.expect("status");
        assert!(!status.remote_configured);
        assert!(!status.needs_remote_repair);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn set_root_path_moves_hydrated_backing_bytes_to_new_root() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        let current_root = dir.path().join("OneDrive");
        let hydrated = current_root
            .join(".openonedrive-cache")
            .join("Docs")
            .join("keep.txt");
        fs::create_dir_all(hydrated.parent().expect("parent")).expect("cache dir");
        fs::write(&hydrated, "cached").expect("cached file");

        let next_root = dir.path().join("OneDrive-Next");
        backend
            .set_root_path(next_root.to_str().expect("utf-8 path"))
            .await
            .expect("set root path");

        let moved = next_root
            .join(".openonedrive-cache")
            .join("Docs")
            .join("keep.txt");
        assert!(moved.exists());
        assert!(!hydrated.exists());
        assert_eq!(backend.current_config().await.root_path, next_root);
    }

    fn build_paths(root: &Path) -> ProjectPaths {
        ProjectPaths {
            config_dir: root.join("config"),
            state_dir: root.join("state"),
            cache_dir: root.join("cache"),
            runtime_dir: root.join("run"),
            config_file: root.join("config").join("config.toml"),
            legacy_db_file: root.join("state").join("state.sqlite3"),
            path_state_db_file: root.join("state").join("path-state.sqlite3"),
            runtime_state_file: root.join("state").join("runtime-state.toml"),
            rclone_config_dir: root.join("config").join("rclone"),
            rclone_config_file: root.join("config").join("rclone").join("rclone.conf"),
            rclone_cache_dir: root.join("cache").join("rclone"),
        }
    }

    async fn build_backend(root: &Path) -> Arc<RcloneBackend> {
        let paths = build_paths(root);
        let mut config = AppConfig::default();
        config.root_path = root.join("OneDrive");
        fs::create_dir_all(&config.root_path).expect("root dir");
        RcloneBackend::load(paths, config).await.expect("backend")
    }
}
