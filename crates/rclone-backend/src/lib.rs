mod path_state;

use anyhow::{Context, Result, bail};
use openonedrive_config::{AppConfig, ProjectPaths, validate_mount_path};
use openonedrive_ipc_types::{MountState, PathState, PathSyncState, StatusSnapshot, SyncState};
use openonedrive_state::{RuntimeState, StateStore};
use path_state::PathStateStore;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock, broadcast};
use tracing::warn;

const MAX_RECENT_LOGS: usize = 200;
const DEFAULT_DIR_CACHE_TIME: &str = "5m";
const DEFAULT_POLL_INTERVAL: &str = "1m";
const DEFAULT_MOUNT_READY_TIMEOUT: Duration = Duration::from_secs(20);
pub const BACKEND_NAME: &str = "rclone";

#[derive(Debug, Clone)]
pub enum BackendEvent {
    MountStateChanged,
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
    mount_state: MountState,
    sync_state: SyncState,
    last_error: String,
    last_sync_error: String,
    last_log_line: String,
    pinned_relative_paths: BTreeSet<String>,
    rclone_version: String,
    mount_desired: bool,
    restart_attempt: u32,
    queue_depth: u32,
    active_transfer_count: u32,
    last_sync_at: u64,
    sync_paused: bool,
}

impl Runtime {
    fn from_state(state: RuntimeState, remote_configured: bool) -> Self {
        let mount_state = if remote_configured {
            match state.mount_state {
                MountState::Mounted | MountState::Mounting | MountState::Connecting => {
                    MountState::Unmounted
                }
                MountState::Disconnected => MountState::Unmounted,
                other => other,
            }
        } else {
            MountState::Disconnected
        };

        Self {
            remote_configured,
            mount_state,
            sync_state: if state.sync_paused {
                SyncState::Paused
            } else {
                state.sync_state
            },
            last_error: state.last_error,
            last_sync_error: state.last_sync_error,
            last_log_line: state.last_log_line,
            pinned_relative_paths: state.pinned_relative_paths.into_iter().collect(),
            rclone_version: String::new(),
            mount_desired: false,
            restart_attempt: 0,
            queue_depth: state.queue_depth,
            active_transfer_count: state.active_transfer_count,
            last_sync_at: state.last_sync_at,
            sync_paused: state.sync_paused,
        }
    }
}

pub struct RcloneBackend {
    paths: ProjectPaths,
    config: RwLock<AppConfig>,
    state_store: StateStore,
    path_state_store: PathStateStore,
    runtime: RwLock<Runtime>,
    recent_logs: Mutex<VecDeque<String>>,
    connect_child: Mutex<Option<Child>>,
    mount_child: Mutex<Option<Child>>,
    connect_generation: Mutex<u64>,
    mount_generation: Mutex<u64>,
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
            recent_logs: Mutex::new(VecDeque::with_capacity(MAX_RECENT_LOGS)),
            connect_child: Mutex::new(None),
            mount_child: Mutex::new(None),
            connect_generation: Mutex::new(0),
            mount_generation: Mutex::new(0),
            event_tx,
        });

        backend.refresh_rclone_version().await;
        if let Err(error) = backend.prune_cache_to_pins().await {
            backend
                .append_log(format!("cache prune skipped: {error}"))
                .await;
        }
        if let Err(error) = backend.refresh_path_state_snapshot().await {
            backend
                .append_log(format!("path state refresh skipped: {error}"))
                .await;
        }

        if backend.current_config().await.auto_mount && remote_configured {
            if let Err(error) = backend.mount().await {
                backend.record_error(error.to_string()).await;
            }
        } else if remote_configured && !backend.runtime.read().await.sync_paused {
            backend.spawn_rescan();
        }

        Ok(backend)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<BackendEvent> {
        self.event_tx.subscribe()
    }

    pub async fn current_config(&self) -> AppConfig {
        self.config.read().await.clone()
    }

    pub async fn get_path_states(&self, raw_paths: &[String]) -> Result<Vec<PathState>> {
        let config = self.current_config().await;
        let mut relative_paths = Vec::with_capacity(raw_paths.len());
        for raw_path in raw_paths {
            let relative = relative_path_for(&config.mount_path, Path::new(raw_path))?;
            relative_paths.push(relative_string(&relative));
        }
        self.path_state_store.get_many(&relative_paths)
    }

    pub async fn get_path_states_json(&self, raw_paths: &[String]) -> Result<String> {
        serde_json::to_string(&self.get_path_states(raw_paths).await?)
            .context("unable to serialize path states")
    }

    pub async fn set_mount_path(self: &Arc<Self>, raw_path: &str) -> Result<()> {
        let requested_path = PathBuf::from(raw_path);
        validate_mount_path(&requested_path)?;

        let should_remount = {
            let runtime = self.runtime.read().await;
            runtime.mount_desired || runtime.mount_state == MountState::Mounted
        };

        self.unmount().await?;

        if !requested_path.exists() {
            fs::create_dir_all(&requested_path)
                .with_context(|| format!("unable to create {}", requested_path.display()))?;
        }

        let mut updated_config = self.current_config().await;
        updated_config.mount_path = requested_path;
        updated_config.save(&self.paths)?;
        *self.config.write().await = updated_config;

        if should_remount && self.runtime.read().await.remote_configured {
            self.mount().await?;
        } else {
            self.persist_runtime().await?;
        }

        Ok(())
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
            runtime.mount_state = MountState::Connecting;
            runtime.last_error.clear();
            runtime.mount_desired = config.auto_mount;
        }
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::MountStateChanged);
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
                            backend.record_error(error.to_string()).await;
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
                            backend.record_error(error.to_string()).await;
                        }
                    }
                    Err(error) => {
                        backend
                            .record_error(format!("waiting for rclone connect failed: {error}"))
                            .await;
                    }
                }
                return;
            }
        });

        Ok(())
    }

    pub async fn disconnect(self: &Arc<Self>) -> Result<()> {
        self.stop_connect_process().await?;
        self.unmount().await?;
        remove_file_if_exists(&self.paths.rclone_config_file)?;
        clear_directory(&self.paths.rclone_cache_dir)?;

        {
            let mut runtime = self.runtime.write().await;
            runtime.remote_configured = false;
            runtime.mount_state = MountState::Disconnected;
            runtime.sync_state = SyncState::Idle;
            runtime.mount_desired = false;
            runtime.restart_attempt = 0;
            runtime.last_error.clear();
            runtime.last_sync_error.clear();
            runtime.last_log_line.clear();
            runtime.pinned_relative_paths.clear();
            runtime.queue_depth = 0;
            runtime.active_transfer_count = 0;
            runtime.last_sync_at = 0;
            runtime.sync_paused = false;
        }
        self.recent_logs.lock().await.clear();
        self.path_state_store.clear()?;
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::MountStateChanged);
        self.emit_event(BackendEvent::SyncStateChanged);
        self.emit_event(BackendEvent::PathStatesChanged(Vec::new()));
        Ok(())
    }

    pub async fn mount(self: &Arc<Self>) -> Result<()> {
        {
            let runtime = self.runtime.read().await;
            if !runtime.remote_configured {
                bail!("no OneDrive remote is configured yet");
            }
            if matches!(
                runtime.mount_state,
                MountState::Mounted | MountState::Mounting
            ) {
                return Ok(());
            }
        }

        let config = self.current_config().await;
        if !config.mount_path.exists() {
            fs::create_dir_all(&config.mount_path)
                .with_context(|| format!("unable to create {}", config.mount_path.display()))?;
        }
        validate_mount_path(&config.mount_path)?;

        let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
        let mut command = Command::new(binary);
        command
            .args(build_mount_args(&config, &self.paths))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().context("failed to spawn rclone mount")?;
        let generation = {
            let mut generation = self.mount_generation.lock().await;
            *generation += 1;
            *generation
        };

        {
            let mut runtime = self.runtime.write().await;
            runtime.mount_state = MountState::Mounting;
            runtime.mount_desired = true;
            runtime.last_error.clear();
            runtime.restart_attempt = 0;
        }
        self.persist_runtime().await?;

        if let Some(stdout) = child.stdout.take() {
            self.spawn_log_reader(stdout, "mount stdout");
        }
        if let Some(stderr) = child.stderr.take() {
            self.spawn_log_reader(stderr, "mount stderr");
        }

        *self.mount_child.lock().await = Some(child);
        self.spawn_mount_ready_waiter(config.mount_path.clone(), generation);

        let backend = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;

                let exit = {
                    let mut slot = backend.mount_child.lock().await;
                    match slot.as_mut() {
                        Some(child) => child.try_wait(),
                        None => return,
                    }
                };

                let current_generation = *backend.mount_generation.lock().await;
                if generation != current_generation {
                    return;
                }

                let exit = match exit {
                    Ok(Some(status)) => {
                        let mut slot = backend.mount_child.lock().await;
                        slot.take();
                        Ok(status)
                    }
                    Ok(None) => continue,
                    Err(error) => {
                        let mut slot = backend.mount_child.lock().await;
                        slot.take();
                        Err(error)
                    }
                };

                match exit {
                    Ok(status) if status.success() => {
                        let desired = backend.runtime.read().await.mount_desired;
                        if desired {
                            backend
                                .record_error("rclone mount exited unexpectedly".to_string())
                                .await;
                            if let Some(delay) = backend.prepare_restart_delay().await {
                                backend.spawn_restart(delay, generation);
                            }
                        } else {
                            backend.set_mount_state(MountState::Unmounted).await;
                        }
                    }
                    Ok(status) => {
                        backend
                            .record_error(format!("rclone mount exited with status {status}"))
                            .await;
                        if let Some(delay) = backend.prepare_restart_delay().await {
                            backend.spawn_restart(delay, generation);
                        }
                    }
                    Err(error) => {
                        backend
                            .record_error(format!("waiting for rclone mount failed: {error}"))
                            .await;
                        if let Some(delay) = backend.prepare_restart_delay().await {
                            backend.spawn_restart(delay, generation);
                        }
                    }
                }
                return;
            }
        });

        Ok(())
    }

    pub async fn unmount(self: &Arc<Self>) -> Result<()> {
        {
            let mut generation = self.mount_generation.lock().await;
            *generation += 1;
        }
        {
            let mut runtime = self.runtime.write().await;
            runtime.mount_desired = false;
            runtime.restart_attempt = 0;
            runtime.last_error.clear();
        }

        if let Some(mut child) = self.mount_child.lock().await.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }

        let mount_state = if self.runtime.read().await.remote_configured {
            MountState::Unmounted
        } else {
            MountState::Disconnected
        };
        self.set_mount_state(mount_state).await;
        self.prune_cache_to_pins().await?;
        self.refresh_path_state_snapshot().await?;
        self.emit_event(BackendEvent::PathStatesChanged(Vec::new()));
        Ok(())
    }

    pub async fn retry_mount(self: &Arc<Self>) -> Result<()> {
        {
            let mut runtime = self.runtime.write().await;
            runtime.last_error.clear();
            runtime.restart_attempt = 0;
        }
        self.persist_runtime().await?;
        self.mount().await
    }

    pub async fn rescan(self: &Arc<Self>) -> Result<u32> {
        {
            let runtime = self.runtime.read().await;
            if !runtime.remote_configured {
                bail!("configure OneDrive before scanning remote state");
            }
        }

        self.begin_sync_activity(SyncState::Scanning).await?;
        let result: Result<u32> = async {
            let entries = self.scan_remote_entries().await?;
            let snapshot = self.build_snapshot_from_remote_entries(&entries).await?;
            let store = self.path_state_store.clone();
            let snapshot_for_store = snapshot.clone();
            tokio::task::spawn_blocking(move || store.replace_all(&snapshot_for_store))
                .await
                .context("path-state write task join failed")??;
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
        self.spawn_rescan();
        Ok(())
    }

    pub async fn keep_local(self: &Arc<Self>, raw_paths: &[String]) -> Result<u32> {
        self.ensure_mounted().await?;
        let config = self.current_config().await;
        let selected_paths = expand_selected_paths(&config.mount_path, raw_paths)?;
        if selected_paths.is_empty() {
            bail!("select at least one file or directory inside the mounted OneDrive path");
        }

        let mount_root = config.mount_path.clone();
        let tracked_paths = selected_paths.iter().cloned().collect::<Vec<_>>();
        self.begin_sync_activity(SyncState::Syncing).await?;
        self.update_selected_path_states(&tracked_paths, PathSyncState::Syncing, String::new())
            .await?;

        let hydrated =
            match tokio::task::spawn_blocking(move || hydrate_paths(&mount_root, selected_paths))
                .await
                .context("keep-local task join failed")?
            {
                Ok(hydrated) => hydrated,
                Err(error) => {
                    self.update_selected_path_states(
                        &tracked_paths,
                        PathSyncState::Error,
                        error.to_string(),
                    )
                    .await?;
                    self.complete_sync_activity(Some(error.to_string())).await?;
                    return Err(error);
                }
            };

        {
            let mut runtime = self.runtime.write().await;
            runtime
                .pinned_relative_paths
                .extend(hydrated.iter().cloned());
        }
        self.persist_runtime().await?;
        self.refresh_path_state_snapshot().await?;
        self.complete_sync_activity(None).await?;
        self.append_log(format!(
            "kept {} item(s) available on this device",
            hydrated.len()
        ))
        .await;
        self.emit_event(BackendEvent::PathStatesChanged(
            hydrated.iter().cloned().collect(),
        ));
        Ok(hydrated.len() as u32)
    }

    pub async fn make_online_only(self: &Arc<Self>, raw_paths: &[String]) -> Result<u32> {
        self.ensure_mounted().await?;
        let config = self.current_config().await;
        let selected_paths = expand_selected_paths(&config.mount_path, raw_paths)?;
        if selected_paths.is_empty() {
            bail!("select at least one file or directory inside the mounted OneDrive path");
        }

        let relative_paths = selected_paths;

        if relative_paths.is_empty() {
            return Ok(0);
        }

        let tracked_paths = relative_paths.iter().cloned().collect::<Vec<_>>();
        self.begin_sync_activity(SyncState::Syncing).await?;
        self.update_selected_path_states(&tracked_paths, PathSyncState::Syncing, String::new())
            .await?;

        {
            let mut runtime = self.runtime.write().await;
            for relative_path in &relative_paths {
                runtime.pinned_relative_paths.remove(relative_path);
            }
        }

        let cache_root = cache_root_for_remote(&self.paths, &config);
        let removed = match tokio::task::spawn_blocking(move || {
            evict_cached_paths(&cache_root, &relative_paths)
        })
        .await
        .context("online-only task join failed")?
        {
            Ok(removed) => removed,
            Err(error) => {
                self.update_selected_path_states(
                    &tracked_paths,
                    PathSyncState::Error,
                    error.to_string(),
                )
                .await?;
                self.complete_sync_activity(Some(error.to_string())).await?;
                return Err(error);
            }
        };

        self.persist_runtime().await?;
        self.refresh_path_state_snapshot().await?;
        self.complete_sync_activity(None).await?;
        self.append_log(format!("returned {} item(s) to online-only mode", removed))
            .await;
        self.emit_event(BackendEvent::PathStatesChanged(tracked_paths));
        Ok(removed)
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
            mount_state: runtime.mount_state,
            sync_state: runtime.sync_state,
            mount_path: config.mount_path.display().to_string(),
            cache_usage_bytes: directory_size_bytes(&self.paths.rclone_cache_dir)?,
            pinned_file_count: runtime.pinned_relative_paths.len() as u32,
            queue_depth: runtime.queue_depth,
            active_transfer_count: runtime.active_transfer_count,
            last_sync_at: runtime.last_sync_at,
            last_sync_error: runtime.last_sync_error,
            rclone_version: runtime.rclone_version,
            last_error: runtime.last_error,
            last_log_line: runtime.last_log_line,
            custom_client_id_configured: config.custom_client_id.is_some(),
        })
    }

    pub async fn recent_log_lines(&self, limit: usize) -> Vec<String> {
        let logs = self.recent_logs.lock().await;
        let skip = logs.len().saturating_sub(limit);
        logs.iter().skip(skip).cloned().collect()
    }

    fn spawn_mount_ready_waiter(self: &Arc<Self>, mount_path: PathBuf, generation: u64) {
        let backend = self.clone();
        tokio::spawn(async move {
            let checks = (DEFAULT_MOUNT_READY_TIMEOUT.as_millis() / 250) as usize;
            for _ in 0..checks.max(1) {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let current_generation = *backend.mount_generation.lock().await;
                if generation != current_generation {
                    return;
                }

                match path_is_mount_point(&mount_path) {
                    Ok(true) => {
                        backend.set_mount_state(MountState::Mounted).await;
                        if let Err(error) = backend.refresh_path_state_snapshot().await {
                            backend
                                .append_log(format!("path state refresh skipped: {error}"))
                                .await;
                        }
                        if !backend.runtime.read().await.sync_paused {
                            backend.spawn_rescan();
                        }
                        return;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        backend.record_error(error.to_string()).await;
                        return;
                    }
                }
            }

            let desired = backend.runtime.read().await.mount_desired;
            let current_generation = *backend.mount_generation.lock().await;
            if desired && generation == current_generation {
                backend
                    .record_error(
                        "rclone mount did not become ready before the timeout".to_string(),
                    )
                    .await;
            }
        });
    }

    fn spawn_rescan(self: &Arc<Self>) {
        let backend = self.clone();
        tokio::spawn(async move {
            if backend.runtime.read().await.sync_paused {
                return;
            }
            if let Err(error) = backend.rescan().await {
                backend
                    .append_log(format!("remote rescan skipped: {error}"))
                    .await;
            }
        });
    }

    async fn begin_sync_activity(&self, sync_state: SyncState) -> Result<()> {
        {
            let mut runtime = self.runtime.write().await;
            runtime.sync_state = sync_state;
            runtime.queue_depth = 1;
            runtime.active_transfer_count = 1;
            runtime.last_sync_error.clear();
        }
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::SyncStateChanged);
        Ok(())
    }

    async fn complete_sync_activity(&self, error: Option<String>) -> Result<()> {
        let mut error_message = None;
        {
            let mut runtime = self.runtime.write().await;
            runtime.queue_depth = 0;
            runtime.active_transfer_count = 0;
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

    async fn update_selected_path_states(
        &self,
        relative_paths: &[String],
        state: PathSyncState,
        error: String,
    ) -> Result<()> {
        if relative_paths.is_empty() {
            return Ok(());
        }

        let existing = {
            let store = self.path_state_store.clone();
            let paths = relative_paths.to_vec();
            tokio::task::spawn_blocking(move || store.get_many(&paths))
                .await
                .context("path-state read task join failed")??
        };
        let existing_map = existing
            .into_iter()
            .map(|path_state| (path_state.path.clone(), path_state))
            .collect::<BTreeMap<_, _>>();

        let config = self.current_config().await;
        let pinned = self.runtime.read().await.pinned_relative_paths.clone();
        let cache_root = cache_root_for_remote(&self.paths, &config);
        let cache_suffixes =
            tokio::task::spawn_blocking(move || cached_suffixes_for_root(&cache_root))
                .await
                .context("cache suffix task join failed")??;
        let sync_at = unix_timestamp();

        let states = relative_paths
            .iter()
            .map(|relative_path| {
                let mut path_state = existing_map
                    .get(relative_path)
                    .cloned()
                    .unwrap_or_else(|| fallback_path_state(&config.mount_path, relative_path));
                path_state.state = state;
                path_state.pinned = pinned.contains(relative_path);
                path_state.cached = cache_suffixes.contains(relative_path);
                path_state.error = error.clone();
                path_state.last_sync_at = sync_at;
                path_state
            })
            .collect::<Vec<_>>();

        let store = self.path_state_store.clone();
        tokio::task::spawn_blocking(move || store.upsert_many(&states))
            .await
            .context("path-state write task join failed")??;
        Ok(())
    }

    async fn refresh_path_state_snapshot(&self) -> Result<()> {
        let existing = {
            let store = self.path_state_store.clone();
            tokio::task::spawn_blocking(move || store.all())
                .await
                .context("path-state read task join failed")??
        };
        if existing.is_empty() {
            return Ok(());
        }

        let config = self.current_config().await;
        let cache_root = cache_root_for_remote(&self.paths, &config);
        let cache_suffixes =
            tokio::task::spawn_blocking(move || cached_suffixes_for_root(&cache_root))
                .await
                .context("cache suffix task join failed")??;
        let runtime = self.runtime.read().await.clone();

        let refreshed =
            rebuild_states_from_runtime(existing, &runtime.pinned_relative_paths, &cache_suffixes);
        let store = self.path_state_store.clone();
        tokio::task::spawn_blocking(move || store.replace_all(&refreshed))
            .await
            .context("path-state write task join failed")??;
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

    async fn build_snapshot_from_remote_entries(
        &self,
        entries: &[RcloneListEntry],
    ) -> Result<Vec<PathState>> {
        let config = self.current_config().await;
        let cache_root = cache_root_for_remote(&self.paths, &config);
        let cache_suffixes =
            tokio::task::spawn_blocking(move || cached_suffixes_for_root(&cache_root))
                .await
                .context("cache suffix task join failed")??;
        let pinned = self.runtime.read().await.pinned_relative_paths.clone();
        Ok(build_snapshot_from_remote_entries(
            entries,
            &pinned,
            &cache_suffixes,
        ))
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
                    Ok(Some(line)) => backend.append_log(format!("{label}: {line}")).await,
                    Ok(None) => break,
                    Err(error) => {
                        backend
                            .append_log(format!("{label}: unable to read process output: {error}"))
                            .await;
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
            if !remote_configured {
                runtime.mount_state = MountState::Disconnected;
            } else if runtime.mount_state == MountState::Disconnected {
                runtime.mount_state = MountState::Unmounted;
            }
            runtime.last_error.clear();
        }
        if let Err(error) = self.persist_runtime().await {
            warn!("unable to persist runtime state: {error:#}");
        }
        self.emit_event(BackendEvent::MountStateChanged);
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
                    ))
                    .await;
                }
                self.set_remote_configured(true).await;
                self.emit_event(BackendEvent::AuthFlowCompleted);
                if config.auto_mount {
                    self.mount().await?;
                } else {
                    self.set_mount_state(MountState::Unmounted).await;
                    if !self.runtime.read().await.sync_paused {
                        self.spawn_rescan();
                    }
                }
                Ok(())
            }
            false => match warning {
                Some(message) => Err(anyhow::anyhow!(message)),
                None => Err(anyhow::anyhow!(
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
            if remote_exists {
                let mount_state = self.runtime.read().await.mount_state;
                if matches!(
                    mount_state,
                    MountState::Disconnected | MountState::Connecting | MountState::Error
                ) {
                    self.set_mount_state(MountState::Unmounted).await;
                }
            }
        }
        Ok(())
    }

    async fn set_mount_state(&self, mount_state: MountState) {
        {
            let mut runtime = self.runtime.write().await;
            runtime.mount_state = mount_state;
            if mount_state != MountState::Error {
                runtime.last_error.clear();
            }
        }
        if let Err(error) = self.persist_runtime().await {
            warn!("unable to persist runtime state: {error:#}");
        }
        self.emit_event(BackendEvent::MountStateChanged);
    }

    async fn record_error(&self, message: String) {
        {
            let mut runtime = self.runtime.write().await;
            runtime.mount_state = MountState::Error;
            runtime.last_error = message.clone();
        }
        self.append_log(message).await;
        if let Err(error) = self.persist_runtime().await {
            warn!("unable to persist runtime state: {error:#}");
        }
        self.emit_event(BackendEvent::MountStateChanged);
        self.emit_event(BackendEvent::ErrorRaised(
            self.runtime.read().await.last_error.clone(),
        ));
    }

    async fn append_log(&self, line: String) {
        let stamped_line = format!("{} {}", log_timestamp(), line);
        {
            let mut logs = self.recent_logs.lock().await;
            if logs.len() == MAX_RECENT_LOGS {
                logs.pop_front();
            }
            logs.push_back(stamped_line.clone());
        }
        {
            let mut runtime = self.runtime.write().await;
            runtime.last_log_line = stamped_line;
        }
        if let Err(error) = self.persist_runtime().await {
            warn!("unable to persist runtime state: {error:#}");
        }
        self.emit_event(BackendEvent::LogsUpdated);
    }

    async fn persist_runtime(&self) -> Result<()> {
        let runtime = self.runtime.read().await;
        self.state_store.save(&RuntimeState {
            remote_configured: runtime.remote_configured,
            mount_state: runtime.mount_state,
            sync_state: runtime.sync_state,
            last_error: runtime.last_error.clone(),
            last_sync_error: runtime.last_sync_error.clone(),
            last_log_line: runtime.last_log_line.clone(),
            pinned_relative_paths: runtime.pinned_relative_paths.iter().cloned().collect(),
            queue_depth: runtime.queue_depth,
            active_transfer_count: runtime.active_transfer_count,
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

    async fn prepare_restart_delay(&self) -> Option<Duration> {
        let mut runtime = self.runtime.write().await;
        if !runtime.mount_desired {
            return None;
        }
        runtime.restart_attempt = runtime.restart_attempt.saturating_add(1);
        Some(restart_backoff(runtime.restart_attempt))
    }

    fn spawn_restart(self: &Arc<Self>, delay: Duration, generation: u64) {
        let backend = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let current_generation = *backend.mount_generation.lock().await;
            let desired = backend.runtime.read().await.mount_desired;
            if generation != current_generation || !desired {
                return;
            }
            if let Err(error) = backend.mount().await {
                backend.record_error(error.to_string()).await;
            }
        });
    }

    async fn ensure_mounted(&self) -> Result<()> {
        let runtime = self.runtime.read().await;
        if runtime.mount_state != MountState::Mounted {
            bail!("mount the OneDrive remote before changing device retention");
        }
        Ok(())
    }

    async fn prune_cache_to_pins(&self) -> Result<()> {
        let config = self.current_config().await;
        let cache_root = cache_root_for_remote(&self.paths, &config);
        let pinned = self.runtime.read().await.pinned_relative_paths.clone();
        tokio::task::spawn_blocking(move || prune_cache_root(&cache_root, &pinned))
            .await
            .context("cache prune task join failed")?
    }
}

fn has_remote_config(config_file: &Path, remote_name: &str) -> Result<bool> {
    if !config_file.exists() {
        return Ok(false);
    }

    let raw = fs::read_to_string(config_file)
        .with_context(|| format!("unable to read {}", config_file.display()))?;
    let marker = format!("[{remote_name}]");
    Ok(raw.lines().any(|line| line.trim() == marker))
}

fn clear_directory(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("unable to remove {}", path.display()))?;
    }
    fs::create_dir_all(path).with_context(|| format!("unable to create {}", path.display()))?;
    Ok(())
}

fn expand_selected_paths(mount_root: &Path, raw_paths: &[String]) -> Result<BTreeSet<String>> {
    let mut files = BTreeSet::new();
    for raw_path in raw_paths {
        let relative = relative_path_for(mount_root, Path::new(raw_path))?;
        let absolute = mount_root.join(&relative);
        collect_selected_files(mount_root, &absolute, &mut files)?;
    }
    Ok(files)
}

fn collect_selected_files(
    mount_root: &Path,
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
            collect_selected_files(mount_root, &entry.path(), files)?;
        }
        return Ok(());
    }

    if metadata.is_file() {
        let relative = relative_path_for(mount_root, path)?;
        files.insert(relative_string(&relative));
    }

    Ok(())
}

fn relative_path_for(mount_root: &Path, raw_path: &Path) -> Result<PathBuf> {
    let absolute = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        mount_root.join(raw_path)
    };
    let relative = absolute.strip_prefix(mount_root).with_context(|| {
        format!(
            "{} is outside the mounted OneDrive path",
            absolute.display()
        )
    })?;
    if relative.as_os_str().is_empty() {
        bail!("select a file or directory inside the mounted OneDrive path");
    }

    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(value) => normalized.push(value),
            std::path::Component::CurDir => {}
            _ => bail!("unsupported path outside the mounted OneDrive path"),
        }
    }

    if normalized.as_os_str().is_empty() {
        bail!("select a file or directory inside the mounted OneDrive path");
    }
    Ok(normalized)
}

fn relative_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn hydrate_paths(mount_root: &Path, relative_paths: BTreeSet<String>) -> Result<BTreeSet<String>> {
    for relative_path in &relative_paths {
        let absolute = mount_root.join(relative_path);
        let mut source = fs::File::open(&absolute)
            .with_context(|| format!("unable to open {}", absolute.display()))?;
        let mut sink = std::io::sink();
        std::io::copy(&mut source, &mut sink)
            .with_context(|| format!("unable to cache {}", absolute.display()))?;
    }
    Ok(relative_paths)
}

fn cache_root_for_remote(paths: &ProjectPaths, config: &AppConfig) -> PathBuf {
    paths.rclone_cache_dir.join("vfs").join(&config.remote_name)
}

fn evict_cached_paths(cache_root: &Path, relative_paths: &BTreeSet<String>) -> Result<u32> {
    if !cache_root.exists() {
        return Ok(relative_paths.len() as u32);
    }

    let mut stack = vec![cache_root.to_path_buf()];
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
                continue;
            }

            let candidate = entry_path
                .strip_prefix(cache_root)
                .with_context(|| format!("unable to relativize {}", entry_path.display()))?;
            if relative_paths
                .iter()
                .any(|relative_path| cache_path_matches(candidate, Path::new(relative_path)))
            {
                fs::remove_file(&entry_path)
                    .with_context(|| format!("unable to remove {}", entry_path.display()))?;
                remove_empty_parent_dirs(&entry_path, cache_root)?;
            }
        }
    }
    Ok(relative_paths.len() as u32)
}

fn prune_cache_root(cache_root: &Path, pinned_relative_paths: &BTreeSet<String>) -> Result<()> {
    if !cache_root.exists() {
        return Ok(());
    }

    let mut stack = vec![cache_root.to_path_buf()];
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
                continue;
            }

            let relative = entry_path
                .strip_prefix(cache_root)
                .with_context(|| format!("unable to relativize {}", entry_path.display()))?;
            if !pinned_relative_paths
                .iter()
                .any(|pinned| cache_path_matches(relative, Path::new(pinned)))
            {
                fs::remove_file(&entry_path)
                    .with_context(|| format!("unable to remove {}", entry_path.display()))?;
            }
        }
    }

    remove_empty_dirs_under(cache_root)?;
    Ok(())
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

fn cache_path_matches(candidate: &Path, selected: &Path) -> bool {
    let candidate_components = candidate
        .components()
        .map(|component| component.as_os_str().to_os_string())
        .collect::<Vec<_>>();
    let selected_components = selected
        .components()
        .map(|component| component.as_os_str().to_os_string())
        .collect::<Vec<_>>();

    if selected_components.len() > candidate_components.len() {
        return false;
    }

    let offset = candidate_components.len() - selected_components.len();
    candidate_components[offset..] == selected_components
}

fn remove_empty_dirs_under(root: &Path) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    let mut dirs = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        dirs.push(path.clone());
        for entry in
            fs::read_dir(&path).with_context(|| format!("unable to inspect {}", path.display()))?
        {
            let entry = entry.with_context(|| format!("unable to read {}", path.display()))?;
            if entry
                .metadata()
                .with_context(|| format!("unable to stat {}", entry.path().display()))?
                .is_dir()
            {
                stack.push(entry.path());
            }
        }
    }

    dirs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for dir in dirs {
        if dir != root {
            match fs::read_dir(&dir) {
                Ok(entries) => {
                    let mut entries = entries;
                    if entries.next().is_none() {
                        fs::remove_dir(&dir)
                            .with_context(|| format!("unable to remove {}", dir.display()))?;
                    }
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("unable to inspect {}", dir.display()));
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct RcloneListEntry {
    #[serde(rename = "Path", default)]
    path: String,
    #[serde(rename = "IsDir", default)]
    is_dir: bool,
    #[serde(rename = "Size", default)]
    size: u64,
}

fn fallback_path_state(mount_root: &Path, relative_path: &str) -> PathState {
    let metadata = fs::metadata(mount_root.join(relative_path)).ok();
    PathState {
        path: relative_path.to_string(),
        is_dir: metadata.as_ref().is_some_and(|metadata| metadata.is_dir()),
        state: PathSyncState::OnlineOnly,
        size_bytes: metadata
            .as_ref()
            .map(|metadata| metadata.len())
            .unwrap_or_default(),
        pinned: false,
        cached: false,
        error: String::new(),
        last_sync_at: unix_timestamp(),
    }
}

fn build_snapshot_from_remote_entries(
    entries: &[RcloneListEntry],
    pinned: &BTreeSet<String>,
    cache_suffixes: &HashSet<String>,
) -> Vec<PathState> {
    let sync_at = unix_timestamp();
    let mut states = BTreeMap::new();

    for entry in entries {
        if entry.path.is_empty() {
            continue;
        }

        let cached = cache_suffixes.contains(&entry.path);
        let pinned_local = pinned.contains(&entry.path);
        states.insert(
            entry.path.clone(),
            PathState {
                path: entry.path.clone(),
                is_dir: entry.is_dir,
                state: if entry.is_dir {
                    PathSyncState::OnlineOnly
                } else {
                    derive_path_state(false, pinned_local, cached)
                },
                size_bytes: entry.size,
                pinned: pinned_local,
                cached,
                error: String::new(),
                last_sync_at: sync_at,
            },
        );
    }

    let file_states = states
        .values()
        .filter(|state| !state.is_dir)
        .cloned()
        .collect::<Vec<_>>();
    apply_directory_states(&mut states, &file_states);
    states.into_values().collect()
}

fn rebuild_states_from_runtime(
    states: Vec<PathState>,
    pinned: &BTreeSet<String>,
    cache_suffixes: &HashSet<String>,
) -> Vec<PathState> {
    let sync_at = unix_timestamp();
    let mut by_path = states
        .into_iter()
        .map(|mut state| {
            if !state.is_dir {
                state.pinned = pinned.contains(&state.path);
                state.cached = cache_suffixes.contains(&state.path);
                state.state =
                    derive_path_state(!state.error.is_empty(), state.pinned, state.cached);
                state.last_sync_at = sync_at;
            }
            (state.path.clone(), state)
        })
        .collect::<BTreeMap<_, _>>();

    let file_states = by_path
        .values()
        .filter(|state| !state.is_dir)
        .cloned()
        .collect::<Vec<_>>();
    apply_directory_states(&mut by_path, &file_states);
    by_path.into_values().collect()
}

fn apply_directory_states(states: &mut BTreeMap<String, PathState>, file_states: &[PathState]) {
    let mut summaries = BTreeMap::<String, (PathSyncState, bool, bool)>::new();

    for file_state in file_states {
        let path = Path::new(&file_state.path);
        let mut current = path.parent();
        while let Some(parent) = current {
            if parent.as_os_str().is_empty() {
                break;
            }
            let key = relative_string(parent);
            let entry = summaries
                .entry(key)
                .or_insert((PathSyncState::OnlineOnly, false, false));
            entry.0 = dominant_path_state(entry.0, file_state.state);
            entry.1 |= file_state.pinned;
            entry.2 |= file_state.cached;
            current = parent.parent();
        }
    }

    for (path, (state, pinned, cached)) in summaries {
        let entry = states.entry(path.clone()).or_insert(PathState {
            path,
            is_dir: true,
            state: PathSyncState::OnlineOnly,
            size_bytes: 0,
            pinned: false,
            cached: false,
            error: String::new(),
            last_sync_at: unix_timestamp(),
        });
        entry.is_dir = true;
        entry.state = state;
        entry.pinned = pinned;
        entry.cached = cached;
        if state != PathSyncState::Error {
            entry.error.clear();
        }
    }
}

fn dominant_path_state(current: PathSyncState, next: PathSyncState) -> PathSyncState {
    fn rank(state: PathSyncState) -> u8 {
        match state {
            PathSyncState::Error => 5,
            PathSyncState::Syncing => 4,
            PathSyncState::PinnedLocal => 3,
            PathSyncState::AvailableLocal => 2,
            PathSyncState::OnlineOnly => 1,
        }
    }

    if rank(next) > rank(current) {
        next
    } else {
        current
    }
}

fn derive_path_state(has_error: bool, pinned: bool, cached: bool) -> PathSyncState {
    if has_error {
        PathSyncState::Error
    } else if pinned {
        PathSyncState::PinnedLocal
    } else if cached {
        PathSyncState::AvailableLocal
    } else {
        PathSyncState::OnlineOnly
    }
}

fn cached_suffixes_for_root(cache_root: &Path) -> Result<HashSet<String>> {
    let mut suffixes = HashSet::new();
    if !cache_root.exists() {
        return Ok(suffixes);
    }

    let mut stack = vec![cache_root.to_path_buf()];
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
                continue;
            }

            let relative = entry_path
                .strip_prefix(cache_root)
                .with_context(|| format!("unable to relativize {}", entry_path.display()))?;
            let components = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>();
            for offset in 0..components.len() {
                suffixes.insert(components[offset..].join("/"));
            }
        }
    }

    Ok(suffixes)
}

fn path_is_mount_point(path: &Path) -> Result<bool> {
    let mountinfo = fs::read_to_string("/proc/self/mountinfo")
        .context("unable to inspect existing mount points")?;
    let canonical = path.to_string_lossy();

    let mounted = mountinfo.lines().any(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        fields.get(4).copied() == Some(canonical.as_ref())
    });

    #[cfg(test)]
    if !mounted && path.join(".openonedrive-mounted").exists() {
        return Ok(true);
    }

    Ok(mounted)
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

pub fn build_mount_args(config: &AppConfig, paths: &ProjectPaths) -> Vec<OsString> {
    vec![
        OsString::from("mount"),
        OsString::from(format!("{}:", config.remote_name)),
        config.mount_path.as_os_str().to_os_string(),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
        OsString::from("--cache-dir"),
        paths.rclone_cache_dir.as_os_str().to_os_string(),
        OsString::from("--vfs-cache-mode"),
        OsString::from("full"),
        OsString::from("--dir-cache-time"),
        OsString::from(DEFAULT_DIR_CACHE_TIME),
        OsString::from("--poll-interval"),
        OsString::from(DEFAULT_POLL_INTERVAL),
        OsString::from("--vfs-cache-max-size"),
        OsString::from(format!("{}G", config.cache_limit_gb)),
    ]
}

pub fn build_lsjson_args(config: &AppConfig, paths: &ProjectPaths) -> Vec<OsString> {
    vec![
        OsString::from("lsjson"),
        OsString::from(format!("{}:", config.remote_name)),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
        OsString::from("--recursive"),
        OsString::from("--no-mimetype"),
    ]
}

pub fn restart_backoff(attempt: u32) -> Duration {
    let capped = attempt.min(5);
    Duration::from_secs(2_u64.saturating_pow(capped))
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now
}

#[cfg(test)]
mod tests {
    use super::{
        RcloneBackend, build_connect_args, build_mount_args, evict_cached_paths,
        expand_selected_paths, prune_cache_root, resolve_rclone_binary_with_path, restart_backoff,
    };
    use openonedrive_config::{AppConfig, ProjectPaths};
    use openonedrive_ipc_types::MountState;
    use std::collections::BTreeSet;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::tempdir;
    use tokio::time::{Duration, sleep};

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
    fn mount_args_use_app_owned_config_and_cache() {
        let dir = tempdir().expect("tempdir");
        let paths = build_paths(dir.path());
        let config = AppConfig {
            mount_path: dir.path().join("mount"),
            cache_limit_gb: 42,
            ..AppConfig::default()
        };

        let args = build_mount_args(&config, &paths);
        let rendered = args
            .iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(rendered.contains(&"mount".to_string()));
        assert!(rendered.contains(&paths.rclone_config_file.display().to_string()));
        assert!(rendered.contains(&paths.rclone_cache_dir.display().to_string()));
        assert!(rendered.contains(&"42G".to_string()));
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
    fn restart_backoff_caps_exponentially() {
        assert_eq!(restart_backoff(1), Duration::from_secs(2));
        assert_eq!(restart_backoff(2), Duration::from_secs(4));
        assert_eq!(restart_backoff(5), Duration::from_secs(32));
        assert_eq!(restart_backoff(8), Duration::from_secs(32));
    }

    #[test]
    fn expands_selected_directories_into_relative_files() {
        let dir = tempdir().expect("tempdir");
        let mount_root = dir.path().join("mount");
        fs::create_dir_all(mount_root.join("docs/nested")).expect("mount tree");
        fs::write(mount_root.join("docs/readme.md"), "a").expect("write file");
        fs::write(mount_root.join("docs/nested/spec.txt"), "b").expect("write file");

        let selected = expand_selected_paths(
            &mount_root,
            &[mount_root.join("docs").display().to_string()],
        )
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
    fn evicts_selected_cached_paths() {
        let dir = tempdir().expect("tempdir");
        let cache_root = dir.path().join("cache");
        fs::create_dir_all(cache_root.join("docs")).expect("cache tree");
        fs::write(cache_root.join("docs/readme.md"), "cached").expect("cached file");

        let removed =
            evict_cached_paths(&cache_root, &BTreeSet::from(["docs/readme.md".to_string()]))
                .expect("evict cache");

        assert_eq!(removed, 1);
        assert!(!cache_root.join("docs/readme.md").exists());
        assert!(!cache_root.join("docs").exists());
    }

    #[test]
    fn prune_cache_root_keeps_only_pinned_files() {
        let dir = tempdir().expect("tempdir");
        let cache_root = dir.path().join("cache");
        fs::create_dir_all(cache_root.join("docs")).expect("cache tree");
        fs::write(cache_root.join("docs/readme.md"), "keep").expect("cached file");
        fs::write(cache_root.join("docs/tmp.log"), "drop").expect("cached file");

        prune_cache_root(&cache_root, &BTreeSet::from(["docs/readme.md".to_string()]))
            .expect("prune cache");

        assert!(cache_root.join("docs/readme.md").exists());
        assert!(!cache_root.join("docs/tmp.log").exists());
    }

    #[tokio::test]
    async fn fake_rclone_can_connect_mount_and_disconnect() {
        let dir = tempdir().expect("tempdir");
        let fake_rclone = dir.path().join("fake-rclone");
        let mount_path = dir.path().join("mount");
        fs::create_dir_all(&mount_path).expect("mount dir");
        write_fake_rclone(&fake_rclone, 0);

        let paths = build_paths(dir.path());
        let config = AppConfig {
            rclone_bin: Some(fake_rclone),
            mount_path,
            ..AppConfig::default()
        };

        let backend = RcloneBackend::load(paths.clone(), config)
            .await
            .expect("backend");
        backend.begin_connect().await.expect("connect");
        sleep(Duration::from_millis(700)).await;

        let status = backend.status().await.expect("status");
        assert!(status.remote_configured);
        assert_eq!(status.mount_state, MountState::Mounted);

        backend.disconnect().await.expect("disconnect");
        let status = backend.status().await.expect("status after disconnect");
        assert!(!status.remote_configured);
        assert_eq!(status.mount_state, MountState::Disconnected);
        assert!(!paths.rclone_config_file.exists());
    }

    #[tokio::test]
    async fn connect_recovers_when_rclone_writes_remote_then_exits_non_zero() {
        let dir = tempdir().expect("tempdir");
        let fake_rclone = dir.path().join("fake-rclone");
        let mount_path = dir.path().join("mount");
        fs::create_dir_all(&mount_path).expect("mount dir");
        write_fake_rclone(&fake_rclone, 2);

        let paths = build_paths(dir.path());
        let config = AppConfig {
            rclone_bin: Some(fake_rclone),
            mount_path,
            ..AppConfig::default()
        };

        let backend = RcloneBackend::load(paths.clone(), config)
            .await
            .expect("backend");
        backend.begin_connect().await.expect("connect");
        sleep(Duration::from_millis(700)).await;

        let status = backend.status().await.expect("status");
        assert!(status.remote_configured);
        assert_eq!(status.mount_state, MountState::Mounted);
        assert!(status.last_error.is_empty());
        assert!(paths.rclone_config_file.exists());
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

    fn write_fake_rclone(path: &Path, connect_exit_code: i32) {
        let script = format!(
            r#"#!/bin/sh
set -eu

cmd="$1"
shift

if [ "$cmd" = "version" ]; then
  echo "rclone v9.9.9"
  exit 0
fi

if [ "$cmd" = "config" ] && [ "$1" = "create" ]; then
  shift
  remote="$1"
  shift
  conf=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --config)
        conf="$2"
        shift 2
        ;;
      *)
        shift
        ;;
    esac
  done
  mkdir -p "$(dirname "$conf")"
  printf '[%s]\ntype = onedrive\n' "$remote" > "$conf"
  echo "config created"
  exit {connect_exit_code}
fi

if [ "$cmd" = "mount" ]; then
  remote="$1"
  mount_path="$2"
  echo "mounted $remote at $mount_path"
  touch "$mount_path/.openonedrive-mounted"
  trap 'rm -f "$mount_path/.openonedrive-mounted"; exit 0' TERM INT
  while true; do
    sleep 1
  done
fi

echo "unexpected invocation" >&2
exit 1
"#
        );

        fs::write(path, script).expect("write fake rclone");
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod");
    }
}
