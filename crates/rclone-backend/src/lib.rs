mod path_state;
mod vfs;

use anyhow::{Context, Result, anyhow, bail};
use openonedrive_config::{AppConfig, ProjectPaths, validate_root_path};
use openonedrive_ipc_types::{
    ConnectionState, FilesystemState, LogEntry, LogLevel, PathState, PathSyncState, StatusSnapshot,
    SyncState,
};
use openonedrive_state::{QueuedActionKind, QueuedActionState, RuntimeState, StateStore};
use path_state::{DirectoryMetadata, PathStateStore};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Condvar, Weak, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock, broadcast};
use tracing::warn;
use vfs::{OpenOneDriveFs, OpenRequest, Provider, SnapshotHandle, VirtualEntry};

const MAX_RECENT_LOGS: usize = 200;
const RESCAN_INTERVAL: Duration = Duration::from_secs(120);
const RECURSIVE_SCAN_TIMEOUT: Duration = Duration::from_secs(10);
const DIRECTORY_LIST_TTL: Duration = Duration::from_secs(2);
pub const BACKEND_NAME: &str = "custom-fuse-rclone";
const OPENONEDRIVE_MOUNT_SOURCE: &str = "openonedrive";
const LEGACY_ONEDRIVE_DRIVE_METADATA_ERROR: &str = "unable to get drive_id and drive_type";
const REMOTE_SCAN_ERROR_PREFIX: &str = "remote scan failed: ";
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
    queued_action_count: u32,
    active_action_kind: String,
    last_sync_at: u64,
    sync_paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MountPointInfo {
    fs_type: String,
    source: String,
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
            queued_action_count: state.queued_actions.len() as u32,
            active_action_kind: state.active_action_kind,
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

#[derive(Debug, Clone)]
enum ActionKind {
    RefreshDirectory {
        path: Option<String>,
        recursive: bool,
    },
    Hydrate {
        path: String,
    },
    Evict {
        path: String,
    },
    Upload {
        path: String,
    },
    CreateDir {
        path: String,
    },
    RemovePath {
        path: String,
        is_dir: bool,
    },
    RenamePath {
        from: String,
        to: String,
    },
}

impl ActionKind {
    fn label(&self) -> &'static str {
        match self {
            Self::RefreshDirectory {
                recursive: true, ..
            } => "refresh-tree",
            Self::RefreshDirectory {
                recursive: false, ..
            } => "refresh-directory",
            Self::Hydrate { .. } => "hydrate",
            Self::Evict { .. } => "evict",
            Self::Upload { .. } => "upload",
            Self::CreateDir { .. } => "mkdir",
            Self::RemovePath { is_dir: true, .. } => "rmdir",
            Self::RemovePath { is_dir: false, .. } => "delete",
            Self::RenamePath { .. } => "rename",
        }
    }

    fn counts_as_download(&self) -> bool {
        matches!(self, Self::Hydrate { .. })
    }

    fn counts_as_upload(&self) -> bool {
        matches!(self, Self::Upload { .. })
    }

    fn counts_as_scan(&self) -> bool {
        matches!(self, Self::RefreshDirectory { .. })
    }

    fn blocked_while_background_sync_is_paused(&self) -> bool {
        matches!(
            self,
            Self::Upload { .. }
                | Self::RefreshDirectory {
                    recursive: true,
                    ..
                }
        )
    }

    fn to_state(&self) -> QueuedActionState {
        match self {
            Self::RefreshDirectory { path, recursive } => QueuedActionState {
                kind: QueuedActionKind::RefreshDirectory,
                path: path.clone().unwrap_or_default(),
                secondary_path: String::new(),
                recursive: *recursive,
            },
            Self::Hydrate { path } => QueuedActionState {
                kind: QueuedActionKind::Hydrate,
                path: path.clone(),
                secondary_path: String::new(),
                recursive: false,
            },
            Self::Evict { path } => QueuedActionState {
                kind: QueuedActionKind::Evict,
                path: path.clone(),
                secondary_path: String::new(),
                recursive: false,
            },
            Self::Upload { path } => QueuedActionState {
                kind: QueuedActionKind::Upload,
                path: path.clone(),
                secondary_path: String::new(),
                recursive: false,
            },
            Self::CreateDir { path } => QueuedActionState {
                kind: QueuedActionKind::CreateDir,
                path: path.clone(),
                secondary_path: String::new(),
                recursive: false,
            },
            Self::RemovePath { path, is_dir } => QueuedActionState {
                kind: if *is_dir {
                    QueuedActionKind::RemoveDir
                } else {
                    QueuedActionKind::RemoveFile
                },
                path: path.clone(),
                secondary_path: String::new(),
                recursive: false,
            },
            Self::RenamePath { from, to } => QueuedActionState {
                kind: QueuedActionKind::RenamePath,
                path: from.clone(),
                secondary_path: to.clone(),
                recursive: false,
            },
        }
    }

    fn from_state(state: QueuedActionState) -> Option<Self> {
        match state.kind {
            QueuedActionKind::RefreshDirectory => Some(Self::RefreshDirectory {
                path: (!state.path.is_empty()).then_some(state.path),
                recursive: state.recursive,
            }),
            QueuedActionKind::Hydrate if !state.path.is_empty() => {
                Some(Self::Hydrate { path: state.path })
            }
            QueuedActionKind::Evict if !state.path.is_empty() => {
                Some(Self::Evict { path: state.path })
            }
            QueuedActionKind::Upload if !state.path.is_empty() => {
                Some(Self::Upload { path: state.path })
            }
            QueuedActionKind::CreateDir if !state.path.is_empty() => {
                Some(Self::CreateDir { path: state.path })
            }
            QueuedActionKind::RemoveFile if !state.path.is_empty() => Some(Self::RemovePath {
                path: state.path,
                is_dir: false,
            }),
            QueuedActionKind::RemoveDir if !state.path.is_empty() => Some(Self::RemovePath {
                path: state.path,
                is_dir: true,
            }),
            QueuedActionKind::RenamePath
                if !state.path.is_empty() && !state.secondary_path.is_empty() =>
            {
                Some(Self::RenamePath {
                    from: state.path,
                    to: state.secondary_path,
                })
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
struct QueuedAction {
    kind: ActionKind,
    responder: Option<mpsc::Sender<Result<()>>>,
}

#[derive(Debug, Default)]
struct ActionScheduler {
    queue: VecDeque<QueuedAction>,
    active_action_kind: String,
    stop_after_current: bool,
}

impl ActionScheduler {
    fn next_runnable_action_index(&self) -> Option<usize> {
        if self.stop_after_current {
            self.queue
                .iter()
                .position(|queued| !queued.kind.blocked_while_background_sync_is_paused())
        } else if self.queue.is_empty() {
            None
        } else {
            Some(0)
        }
    }

    fn pop_next_runnable_action(&mut self) -> Option<QueuedAction> {
        let index = self.next_runnable_action_index()?;
        self.queue.remove(index)
    }
}

#[derive(Debug, Default)]
struct RemoteScanResult {
    entries: Vec<RcloneListEntry>,
    failed_directories: BTreeMap<String, String>,
    listed_directories: BTreeSet<String>,
}

pub struct RcloneBackend {
    paths: ProjectPaths,
    config: RwLock<AppConfig>,
    state_store: StateStore,
    path_state_store: PathStateStore,
    runtime: RwLock<Runtime>,
    recent_logs: std::sync::Mutex<VecDeque<LogEntry>>,
    rclone_process_lock: Mutex<()>,
    connect_child: Mutex<Option<Child>>,
    connect_generation: Mutex<u64>,
    rescan_generation: Mutex<u64>,
    action_scheduler: Arc<(std::sync::Mutex<ActionScheduler>, Condvar)>,
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
        let restored_actions = persisted
            .queued_actions
            .clone()
            .into_iter()
            .filter_map(ActionKind::from_state)
            .map(|kind| QueuedAction {
                kind,
                responder: None,
            })
            .collect::<VecDeque<_>>();

        let backend = Arc::new(Self {
            paths,
            config: RwLock::new(config),
            state_store,
            path_state_store,
            runtime: RwLock::new(Runtime::from_state(persisted, remote_configured)),
            recent_logs: std::sync::Mutex::new(VecDeque::with_capacity(MAX_RECENT_LOGS)),
            rclone_process_lock: Mutex::new(()),
            connect_child: Mutex::new(None),
            connect_generation: Mutex::new(0),
            rescan_generation: Mutex::new(0),
            action_scheduler: Arc::new((
                std::sync::Mutex::new(ActionScheduler {
                    queue: restored_actions,
                    active_action_kind: String::new(),
                    stop_after_current: false,
                }),
                Condvar::new(),
            )),
            filesystem_session: std::sync::Mutex::new(None),
            underlay_root: std::sync::Mutex::new(None),
            snapshot: SnapshotHandle::default(),
            runtime_handle: tokio::runtime::Handle::current(),
            event_tx,
        });

        backend.ensure_visible_root_exists();
        backend.spawn_action_worker();
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

    fn spawn_action_worker(self: &Arc<Self>) {
        let weak_backend: Weak<Self> = Arc::downgrade(self);
        let scheduler = self.action_scheduler.clone();
        std::thread::spawn(move || {
            loop {
                let queued = {
                    let (lock, wake) = &*scheduler;
                    let mut state = lock.lock().expect("action scheduler poisoned");
                    while state.next_runnable_action_index().is_none() {
                        state.active_action_kind.clear();
                        state = wake.wait(state).expect("action scheduler poisoned");
                    }
                    let queued = state.pop_next_runnable_action().expect("queue entry");
                    state.active_action_kind = queued.kind.label().to_string();
                    queued
                };

                let Some(backend) = weak_backend.upgrade() else {
                    return;
                };
                let _ = backend.sync_runtime_from_action_scheduler_blocking();
                let result = backend.process_queued_action(&queued.kind);
                {
                    let mut state = backend
                        .action_scheduler
                        .0
                        .lock()
                        .expect("action scheduler poisoned");
                    if state.active_action_kind == queued.kind.label() {
                        state.active_action_kind.clear();
                    }
                }
                let _ = backend.sync_runtime_from_action_scheduler_blocking();
                if let Some(responder) = queued.responder {
                    let _ = responder.send(result);
                }
            }
        });
    }

    async fn run_queued_action(&self, action: ActionKind) -> Result<()> {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.run_queued_action_sync(action))
        } else {
            self.run_queued_action_sync(action)
        }
    }

    fn run_queued_action_sync(&self, action: ActionKind) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        {
            let mut state = self
                .action_scheduler
                .0
                .lock()
                .expect("action scheduler poisoned");
            state.queue.push_back(QueuedAction {
                kind: action,
                responder: Some(tx),
            });
            self.action_scheduler.1.notify_one();
        }
        self.sync_runtime_from_action_scheduler_blocking()?;
        rx.recv()
            .map_err(|_| anyhow!("queued action worker exited before completing the request"))?
    }

    fn enqueue_background_action(self: &Arc<Self>, action: ActionKind) -> Result<()> {
        {
            let mut state = self
                .action_scheduler
                .0
                .lock()
                .expect("action scheduler poisoned");
            state.queue.push_back(QueuedAction {
                kind: action,
                responder: None,
            });
            self.action_scheduler.1.notify_one();
        }
        self.sync_runtime_from_action_scheduler_blocking()?;
        Ok(())
    }

    fn process_queued_action(&self, action: &ActionKind) -> Result<()> {
        match action {
            ActionKind::RefreshDirectory { path, recursive } => self
                .runtime_handle
                .block_on(self.refresh_directory_action(path.clone(), *recursive)),
            ActionKind::Hydrate { path } => {
                self.hydrate_relative_path_sync(path)?;
                self.rebuild_path_state_snapshot_sync()?;
                self.emit_path_state_refresh(std::slice::from_ref(path));
                Ok(())
            }
            ActionKind::Evict { path } => {
                self.evict_relative_path_sync(path)?;
                self.rebuild_path_state_snapshot_sync()?;
                self.emit_path_state_refresh(std::slice::from_ref(path));
                Ok(())
            }
            ActionKind::Upload { path } => {
                if let Err(error) = self.upload_relative_path_sync(path) {
                    self.set_path_error_sync(path, error.to_string());
                    return Ok(());
                }
                self.rebuild_path_state_snapshot_sync()?;
                self.emit_path_state_refresh(std::slice::from_ref(path));
                self.append_log_entry("sync", LogLevel::Info, format!("uploaded {path}"));
                Ok(())
            }
            ActionKind::CreateDir { path } => {
                self.create_remote_dir_sync(path)?;
                self.rebuild_path_state_snapshot_sync()?;
                self.emit_path_state_refresh(std::slice::from_ref(path));
                Ok(())
            }
            ActionKind::RemovePath { path, is_dir } => {
                self.remove_path_sync(path, *is_dir)?;
                self.rebuild_path_state_snapshot_sync()?;
                self.emit_path_state_refresh(std::slice::from_ref(path));
                Ok(())
            }
            ActionKind::RenamePath { from, to } => {
                self.rename_path_sync(from, to)?;
                self.rebuild_path_state_snapshot_sync()?;
                self.emit_path_state_refresh(&[from.clone(), to.clone()]);
                Ok(())
            }
        }
    }

    async fn refresh_directory_action(&self, path: Option<String>, recursive: bool) -> Result<()> {
        if recursive {
            return self.run_recursive_refresh_action().await;
        }

        self.refresh_directory_listing_direct(path.as_deref()).await
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

    pub async fn list_directory(&self, raw_path: &str) -> Result<Vec<PathState>> {
        let config = self.current_config().await;
        let target = normalize_directory_query_path(&config.root_path, raw_path)?;
        self.ensure_directory_listing(target.as_deref()).await?;
        let mut entries = self
            .path_state_store
            .all()?
            .into_iter()
            .filter(|state| immediate_child_of(&state.path, target.as_deref()))
            .collect::<Vec<_>>();
        sort_path_states(&mut entries);
        Ok(entries)
    }

    pub async fn list_directory_json(&self, raw_path: &str) -> Result<String> {
        serde_json::to_string(&self.list_directory(raw_path).await?)
            .context("unable to serialize directory listing")
    }

    pub async fn refresh_directory(&self, raw_path: &str) -> Result<u32> {
        let config = self.current_config().await;
        let target = normalize_directory_query_path(&config.root_path, raw_path)?;
        self.refresh_directory_listing(target.as_deref()).await?;
        Ok(self
            .path_state_store
            .all()?
            .into_iter()
            .filter(|state| immediate_child_of(&state.path, target.as_deref()))
            .count() as u32)
    }

    pub async fn search_paths(&self, query: &str, limit: usize) -> Result<Vec<PathState>> {
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        if self.path_state_store.all()?.is_empty() {
            self.ensure_directory_listing(None).await?;
        }

        let mut matches = self
            .path_state_store
            .all()?
            .into_iter()
            .filter(|state| path_matches_query(state, &normalized_query))
            .collect::<Vec<_>>();
        sort_path_states(&mut matches);
        matches.truncate(limit);
        Ok(matches)
    }

    pub async fn search_paths_json(&self, query: &str, limit: usize) -> Result<String> {
        serde_json::to_string(&self.search_paths(query, limit).await?)
            .context("unable to serialize search results")
    }

    async fn ensure_directory_listing(&self, relative_path: Option<&str>) -> Result<()> {
        let Some(refresh_path) = self.directory_listing_refresh_path(relative_path)? else {
            return Ok(());
        };
        self.refresh_directory_listing(Some(refresh_path.as_str()))
            .await
    }

    fn directory_listing_refresh_path(
        &self,
        relative_path: Option<&str>,
    ) -> Result<Option<String>> {
        let listing_key = relative_path.unwrap_or_default();
        let metadata = self.path_state_store.directory_metadata(listing_key)?;
        let needs_refresh = metadata.is_none_or(|metadata| {
            !metadata.children_known
                || metadata
                    .last_listed_at
                    .saturating_add(DIRECTORY_LIST_TTL.as_secs())
                    < unix_timestamp()
        });
        if needs_refresh {
            Ok(Some(listing_key.to_string()))
        } else {
            Ok(None)
        }
    }

    async fn refresh_directory_listing(&self, relative_path: Option<&str>) -> Result<()> {
        self.run_queued_action(ActionKind::RefreshDirectory {
            path: relative_path.map(ToOwned::to_owned),
            recursive: false,
        })
        .await
    }

    async fn refresh_directory_listing_direct(&self, relative_path: Option<&str>) -> Result<()> {
        self.reconcile_remote_state_from_disk().await?;
        if !self.runtime.read().await.remote_configured {
            return Ok(());
        }
        if let Some(relative_path) = relative_path {
            let state = self
                .path_state_store
                .get_many(&[relative_path.to_string()])?
                .into_iter()
                .next();
            if state.as_ref().is_some_and(|state| !state.is_dir) {
                return Ok(());
            }
        }

        let config = self.current_config().await;
        let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
        let scope = match relative_path {
            Some(path) => format!("directory refresh {path}"),
            None => "directory refresh root".to_string(),
        };
        let mut entries = self
            .run_lsjson_command(
                binary.as_path(),
                build_lsjson_directory_args(&config, &self.paths, relative_path, false),
                &scope,
                None,
            )
            .await?;
        if let Some(relative_path) = relative_path {
            entries = prefix_lsjson_entries(relative_path, entries);
        }

        let updated_states = entries
            .iter()
            .map(|entry| self.path_state_from_remote_entry(entry))
            .collect::<Result<Vec<_>>>()?;
        self.path_state_store.upsert_many(&updated_states)?;
        self.path_state_store.set_directory_metadata(
            relative_path.unwrap_or_default(),
            true,
            unix_timestamp(),
        )?;
        self.rebuild_path_state_snapshot_sync()?;
        let mut changed = updated_states
            .into_iter()
            .map(|state| state.path)
            .collect::<Vec<_>>();
        if let Some(relative_path) = relative_path {
            changed.push(relative_path.to_string());
        }
        self.emit_path_state_refresh(&changed);
        Ok(())
    }

    fn path_state_from_remote_entry(&self, entry: &RcloneListEntry) -> Result<PathState> {
        let existing_state = self
            .path_state_store
            .get_many(&[entry.path.clone()])?
            .into_iter()
            .next();
        let hydrated = if entry.is_dir {
            existing_state.as_ref().is_some_and(|state| state.hydrated)
        } else {
            self.backing_file_path(&entry.path)
                .ok()
                .is_some_and(|path| path.exists())
        };
        let pinned = existing_state.as_ref().is_some_and(|state| state.pinned);
        let dirty = existing_state.as_ref().is_some_and(|state| state.dirty);
        let error = existing_state
            .as_ref()
            .map(|state| clear_remote_scan_error(&state.error))
            .unwrap_or_default();
        let conflict_reason = existing_state
            .as_ref()
            .map(|state| state.conflict_reason.clone())
            .unwrap_or_default();
        let modified_unix = if dirty {
            existing_state
                .as_ref()
                .map(|state| state.last_sync_at)
                .unwrap_or_else(unix_timestamp)
        } else {
            parse_rclone_mod_time_unix(&entry.mod_time)
                .or_else(|| existing_state.as_ref().map(|state| state.last_sync_at))
                .unwrap_or_else(unix_timestamp)
        };

        let mut state = PathState {
            path: entry.path.clone(),
            is_dir: entry.is_dir,
            state: PathSyncState::OnlineOnly,
            size_bytes: entry.size_bytes(),
            pinned,
            hydrated,
            dirty,
            error,
            last_sync_at: modified_unix,
            base_revision: revision_for_entry(entry),
            conflict_reason,
        };
        state.state = derive_path_state(&state);
        Ok(state)
    }

    pub async fn set_root_path(self: &Arc<Self>, raw_path: &str) -> Result<()> {
        let requested_path = PathBuf::from(raw_path);
        let mut updated_config = self.current_config().await;
        let previous_root_path = updated_config.root_path.clone();
        validate_root_path(&requested_path, &updated_config.backing_dir_name)?;
        self.pause_uploads_for_lifecycle(true).await?;

        let should_restart = self.runtime.read().await.filesystem_state == FilesystemState::Running;
        self.stop_filesystem().await?;

        create_dir_all_with_reason(&requested_path)?;
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
        if !self.runtime.read().await.sync_paused && self.runtime.read().await.remote_configured {
            self.resume_upload_queue();
            self.enqueue_dirty_uploads()?;
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
        self.pause_uploads_for_lifecycle(true).await?;
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
        if self.cleanup_stale_mountpoint(&config.root_path).await? {
            self.append_log(format!(
                "cleared stale filesystem mountpoint at {}",
                config.root_path.display()
            ));
        }
        validate_root_path(&config.root_path, &config.backing_dir_name)?;
        create_dir_all_with_reason(&config.root_path)?;
        create_dir_all_with_reason(&config.backing_dir_path())?;

        {
            let mut runtime = self.runtime.write().await;
            runtime.filesystem_state = FilesystemState::Starting;
            runtime.last_error.clear();
        }
        self.persist_runtime().await?;
        self.emit_event(BackendEvent::FilesystemStateChanged);
        let result: Result<()> = async {
            if self.path_state_store.all()?.is_empty() {
                if let Err(error) = self.seed_root_snapshot_preview().await {
                    self.append_warning_log(
                        "filesystem",
                        format!("unable to seed root listing before mount: {error}"),
                    );
                    self.refresh_virtual_snapshot()?;
                }
            } else {
                self.refresh_virtual_snapshot()?;
            }

            let root_handle = File::open(&config.root_path)
                .with_context(|| format!("unable to open {}", config.root_path.display()))?;
            set_file_descriptor_inheritable(&root_handle)?;
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
                self.spawn_rescan("filesystem-start");
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

    async fn cleanup_stale_mountpoint(&self, root_path: &Path) -> Result<bool> {
        let Some(info) = mount_point_info(root_path)? else {
            return Ok(false);
        };
        if !is_openonedrive_mount(&info) || !mountpoint_is_stale(root_path) {
            return Ok(false);
        }

        self.append_warning_log(
            "filesystem",
            format!(
                "stale filesystem mountpoint detected at {}; attempting cleanup",
                root_path.display()
            ),
        );
        unmount_mountpoint(root_path).await?;
        Ok(true)
    }

    pub async fn rescan(self: &Arc<Self>) -> Result<u32> {
        self.run_queued_action(ActionKind::RefreshDirectory {
            path: None,
            recursive: true,
        })
        .await?;
        Ok(self.path_state_store.all()?.len() as u32)
    }

    async fn run_recursive_refresh_action(&self) -> Result<()> {
        if let Err(error) = self.ensure_remote_ready_for_use().await {
            let message = error.to_string();
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
            let existing_states = self.path_state_store.all()?;
            let store_is_empty = existing_states.is_empty();
            let seed_progressively = store_is_empty
                || existing_states
                    .iter()
                    .all(|state| !state.path.contains('/'));
            if store_is_empty {
                if let Err(error) = self.seed_root_snapshot_preview().await {
                    self.append_warning_log(
                        "sync",
                        format!("unable to seed root listing before full scan: {error}"),
                    );
                }
            }
            let remote_scan = self.scan_remote_entries(seed_progressively).await?;
            let mut snapshot = self.build_snapshot_from_remote_entries(&remote_scan.entries)?;
            apply_failed_remote_scan_directories(&mut snapshot, &remote_scan.failed_directories);
            let store = self.path_state_store.clone();
            let snapshot_for_store = snapshot.clone();
            tokio::task::spawn_blocking(move || store.replace_all(&snapshot_for_store))
                .await
                .context("path-state write task join failed")??;
            self.path_state_store.set_directory_metadata_many(
                &directory_metadata_for_listed_directories(&remote_scan.listed_directories),
            )?;
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

        result.map(|_| ())
    }

    pub async fn pause_sync(&self) -> Result<()> {
        self.stop_rescan_loop().await;
        self.pause_uploads_for_lifecycle(true).await?;
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
        self.resume_upload_queue();
        self.restart_rescan_loop().await;
        self.enqueue_dirty_uploads()?;
        self.spawn_rescan("resume");
        Ok(())
    }

    pub async fn keep_local(self: &Arc<Self>, raw_paths: &[String]) -> Result<u32> {
        let config = self.current_config().await;
        let states = self.path_state_store.all()?;
        self.begin_sync_activity(SyncState::Syncing)?;
        let selected_paths = match expand_selected_paths(&config.root_path, raw_paths, &states) {
            Ok(selected_paths) if !selected_paths.is_empty() => selected_paths,
            Ok(_) => {
                let message =
                    "select at least one file or directory inside the OneDrive folder".to_string();
                self.complete_sync_activity(Some(message.clone())).await?;
                bail!(message);
            }
            Err(error) => {
                let message = error.to_string();
                self.complete_sync_activity(Some(message.clone())).await?;
                return Err(error);
            }
        };

        let mut changed = Vec::new();
        for relative_path in &selected_paths {
            if let Err(error) = self
                .run_queued_action(ActionKind::Hydrate {
                    path: relative_path.clone(),
                })
                .await
            {
                if !changed.is_empty() {
                    let _ = self.rebuild_path_state_snapshot_sync();
                    self.emit_path_state_refresh(&changed);
                }
                self.complete_sync_activity(Some(error.to_string())).await?;
                return Err(error);
            }
            changed.push(relative_path.clone());
        }

        self.set_pinned_state(&changed, true)?;
        self.rebuild_path_state_snapshot_sync()?;
        self.complete_sync_activity(None).await?;
        self.append_log(format!(
            "kept {} item(s) available on this device",
            changed.len()
        ));
        self.emit_path_state_refresh(&changed);
        Ok(changed.len() as u32)
    }

    pub async fn make_online_only(self: &Arc<Self>, raw_paths: &[String]) -> Result<u32> {
        let config = self.current_config().await;
        let states = self.path_state_store.all()?;

        self.begin_sync_activity(SyncState::Syncing)?;
        let mut changed = Vec::new();
        let selected_paths = match expand_selected_paths(&config.root_path, raw_paths, &states) {
            Ok(selected_paths) if !selected_paths.is_empty() => selected_paths,
            Ok(_) => {
                let message =
                    "select at least one file or directory inside the OneDrive folder".to_string();
                self.complete_sync_activity(Some(message.clone())).await?;
                bail!(message);
            }
            Err(error) => {
                let message = error.to_string();
                self.complete_sync_activity(Some(message.clone())).await?;
                return Err(error);
            }
        };
        let state_map = states
            .iter()
            .map(|state| (state.path.clone(), state))
            .collect::<HashMap<_, _>>();
        for relative_path in &selected_paths {
            let Some(state) = state_map.get(relative_path) else {
                let error = anyhow!("unknown path {relative_path}");
                self.complete_sync_activity(Some(error.to_string())).await?;
                return Err(error);
            };
            if !state.is_dir && (state.dirty || state.state == PathSyncState::Conflict) {
                let error = anyhow!("cannot evict {} while it has local changes", relative_path);
                self.complete_sync_activity(Some(error.to_string())).await?;
                return Err(error);
            }
        }

        let eviction_order = sort_paths_for_eviction(&selected_paths, &state_map);
        let result: Result<u32> = async {
            for relative_path in &eviction_order {
                self.run_queued_action(ActionKind::Evict {
                    path: relative_path.clone(),
                })
                .await?;
                changed.push(relative_path.clone());
            }
            Ok(changed.len() as u32)
        }
        .await;

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
            queued_action_count: runtime.queued_action_count,
            active_action_kind: runtime.active_action_kind.clone(),
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
        logs.iter().skip(skip).map(format_log_entry).collect()
    }

    pub async fn recent_logs(&self, limit: usize) -> Vec<LogEntry> {
        let logs = self.recent_logs.lock().expect("logs poisoned");
        let skip = logs.len().saturating_sub(limit);
        logs.iter().skip(skip).cloned().collect()
    }

    pub async fn recent_logs_json(&self, limit: usize) -> Result<String> {
        serde_json::to_string(&self.recent_logs(limit).await)
            .context("unable to serialize recent logs")
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
        self.pause_uploads_for_lifecycle(true).await?;
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
                runtime.sync_state = if runtime.pending_downloads > 0 || runtime.pending_uploads > 0
                {
                    SyncState::Syncing
                } else if runtime.sync_paused {
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
            state.state = derive_path_state(&state);
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

    fn ensure_visible_root_exists(&self) {
        let root_path = self.config_read_guard().root_path.clone();
        match create_root_path_if_missing(&root_path) {
            Ok(true) => self.append_log(format!(
                "prepared visible root folder at {}",
                root_path.display()
            )),
            Ok(false) => {}
            Err(error) => self.append_warning_log(
                "filesystem",
                format!(
                    "unable to prepare visible root folder {}: {error}",
                    root_path.display()
                ),
            ),
        }
    }

    fn refresh_virtual_snapshot(&self) -> Result<()> {
        let states = self.path_state_store.all()?;
        self.refresh_virtual_snapshot_with_states(&states);
        Ok(())
    }

    async fn scan_remote_entries(&self, seed_progressively: bool) -> Result<RemoteScanResult> {
        let config = self.current_config().await;
        let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
        match self
            .run_lsjson_command(
                &binary,
                build_lsjson_args(&config, &self.paths),
                "remote scan",
                Some(RECURSIVE_SCAN_TIMEOUT),
            )
            .await
        {
            Ok(entries) => Ok(RemoteScanResult {
                listed_directories: fully_listed_directories_for_entries(&entries),
                entries,
                failed_directories: BTreeMap::new(),
            }),
            Err(error) => {
                if is_legacy_onedrive_remote_error(&error.to_string()) {
                    return Err(error);
                }
                self.append_warning_log(
                    "sync",
                    format!("recursive remote scan failed, retrying directory crawl: {error}"),
                );
                self.scan_remote_entries_by_directory(&config, binary.as_path(), seed_progressively)
                    .await
            }
        }
    }

    async fn scan_remote_entries_by_directory(
        &self,
        config: &AppConfig,
        binary: &Path,
        seed_progressively: bool,
    ) -> Result<RemoteScanResult> {
        let root_entries = self
            .run_lsjson_command(
                binary,
                build_lsjson_directory_args(config, &self.paths, None, false),
                "remote root",
                None,
            )
            .await?;
        let mut result = RemoteScanResult::default();
        result.listed_directories.insert(String::new());
        let mut queue = VecDeque::new();

        for entry in root_entries {
            if entry.path.is_empty() {
                continue;
            }
            if entry.is_dir {
                queue.push_back(entry.path.clone());
            }
            result.entries.push(entry);
        }

        while let Some(relative_path) = queue.pop_front() {
            match self
                .run_lsjson_command(
                    binary,
                    build_lsjson_directory_args(config, &self.paths, Some(&relative_path), false),
                    &format!("remote directory {relative_path}"),
                    None,
                )
                .await
            {
                Ok(children) => {
                    result.listed_directories.insert(relative_path.clone());
                    for child in prefix_lsjson_entries(&relative_path, children) {
                        if child.path.is_empty() {
                            continue;
                        }
                        if child.is_dir {
                            queue.push_back(child.path.clone());
                        }
                        result.entries.push(child);
                    }
                }
                Err(error) => {
                    let message = mark_remote_scan_error(&error.to_string());
                    self.append_warning_log(
                        "sync",
                        format!("skipping remote subtree {relative_path}: {message}"),
                    );
                    result
                        .failed_directories
                        .insert(relative_path.clone(), message);
                }
            }

            if seed_progressively {
                self.persist_remote_scan_progress(
                    &result.entries,
                    &result.failed_directories,
                    &result.listed_directories,
                )
                .await?;
            }
        }

        if !result.failed_directories.is_empty() {
            self.append_warning_log(
                "sync",
                format!(
                    "remote scan skipped {} subtree(s) with listing errors",
                    result.failed_directories.len()
                ),
            );
        }

        Ok(result)
    }

    async fn run_lsjson_command(
        &self,
        binary: &Path,
        args: Vec<OsString>,
        scope: &str,
        timeout: Option<Duration>,
    ) -> Result<Vec<RcloneListEntry>> {
        let output = self
            .run_rclone_output_command(binary, args, scope, timeout)
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if is_legacy_onedrive_remote_error(&stderr) {
                bail!("{}", build_remote_repair_message(&stderr));
            }
            bail!(
                "rclone lsjson failed for {scope}{}",
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

    async fn run_rclone_output_command(
        &self,
        binary: &Path,
        args: Vec<OsString>,
        scope: &str,
        timeout: Option<Duration>,
    ) -> Result<std::process::Output> {
        let _guard = self.rclone_process_lock.lock().await;
        let output_future = Command::new(binary)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output();
        let output = match timeout {
            Some(timeout) => tokio::time::timeout(timeout, output_future)
                .await
                .with_context(|| format!("rclone command timed out for {scope}"))??,
            None => output_future
                .await
                .with_context(|| format!("failed to execute rclone command for {scope}"))?,
        };
        Ok(output)
    }

    async fn seed_root_snapshot_preview(&self) -> Result<()> {
        let config = self.current_config().await;
        let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
        let root_entries = self
            .run_lsjson_command(
                binary.as_path(),
                build_lsjson_directory_args(&config, &self.paths, None, false),
                "remote root preview",
                None,
            )
            .await?;
        if root_entries.is_empty() {
            return Ok(());
        }

        let snapshot = self.build_snapshot_from_remote_entries(&root_entries)?;
        let store = self.path_state_store.clone();
        let snapshot_for_store = snapshot.clone();
        tokio::task::spawn_blocking(move || store.replace_all(&snapshot_for_store))
            .await
            .context("path-state preview write task join failed")??;
        self.path_state_store
            .set_directory_metadata_many(&[DirectoryMetadata {
                path: String::new(),
                children_known: true,
                last_listed_at: unix_timestamp(),
            }])?;
        self.refresh_virtual_snapshot_with_states(&snapshot);
        self.sync_runtime_sets_from_states(&snapshot)?;
        self.emit_event(BackendEvent::PathStatesChanged(Vec::new()));
        Ok(())
    }

    async fn persist_remote_scan_progress(
        &self,
        entries: &[RcloneListEntry],
        failed_directories: &BTreeMap<String, String>,
        listed_directories: &BTreeSet<String>,
    ) -> Result<()> {
        let mut snapshot = self.build_snapshot_from_remote_entries(entries)?;
        apply_failed_remote_scan_directories(&mut snapshot, failed_directories);
        let store = self.path_state_store.clone();
        let snapshot_for_store = snapshot.clone();
        tokio::task::spawn_blocking(move || store.replace_all(&snapshot_for_store))
            .await
            .context("path-state progress write task join failed")??;
        self.path_state_store.set_directory_metadata_many(
            &directory_metadata_for_listed_directories(listed_directories),
        )?;
        self.refresh_virtual_snapshot_with_states(&snapshot);
        self.sync_runtime_sets_from_states(&snapshot)?;
        self.emit_event(BackendEvent::PathStatesChanged(Vec::new()));
        Ok(())
    }

    fn fetch_remote_file_entry_sync(&self, relative_path: &str) -> Result<Option<RcloneListEntry>> {
        let config = self.config_read_guard().clone();
        let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
        let output = self.run_rclone_output_blocking(
            binary.as_path(),
            build_lsjson_single_args(&config, &self.paths, relative_path),
            "failed to execute rclone lsjson for single path",
        )?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            bail!(
                "rclone lsjson failed for {}{}",
                relative_path,
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {stderr}")
                }
            );
        }

        let payload =
            String::from_utf8(output.stdout).context("rclone lsjson returned invalid utf-8")?;
        let entries = serde_json::from_str::<Vec<RcloneListEntry>>(&payload)
            .context("unable to parse rclone lsjson output")?;
        let file_name = path_name(relative_path);
        Ok(entries.into_iter().find(|entry| {
            entry.path == relative_path
                || entry.path == file_name
                || entry.path.trim_start_matches('/') == relative_path
        }))
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
                .map(|state| clear_remote_scan_error(&state.error))
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
                size_bytes: entry.size_bytes(),
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
            .and_then(|binary| self.read_rclone_version_blocking(binary))
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
                    Ok(Some(line)) => backend.append_warning_log(label, line),
                    Ok(None) => break,
                    Err(error) => {
                        backend.append_error_log(
                            label,
                            format!("unable to read process output: {error}"),
                        );
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
        self.append_error_log("connection", message.clone());
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
        self.append_error_log("filesystem", message.clone());
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
        self.append_error_log("filesystem", message.clone());
        self.emit_event(BackendEvent::FilesystemStateChanged);
        self.emit_event(BackendEvent::ConnectionStateChanged);
        self.emit_event(BackendEvent::ErrorRaised(message));
        Ok(())
    }

    fn append_log_entry(&self, source: &str, level: LogLevel, message: String) {
        let entry = LogEntry {
            timestamp_unix: unix_timestamp(),
            source: source.to_string(),
            level,
            message,
        };
        let formatted = format_log_entry(&entry);
        {
            let mut logs = self.recent_logs.lock().expect("logs poisoned");
            if logs.len() == MAX_RECENT_LOGS {
                logs.pop_front();
            }
            logs.push_back(entry);
        }
        {
            let mut runtime = self.runtime_write_guard();
            runtime.last_log_line = formatted;
        }
        if let Err(error) = self.persist_runtime_blocking() {
            warn!("unable to persist runtime state: {error:#}");
        }
        self.emit_event(BackendEvent::LogsUpdated);
    }

    fn append_log(&self, line: String) {
        self.append_log_entry("daemon", LogLevel::Info, line);
    }

    fn append_warning_log(&self, source: &str, line: String) {
        self.append_log_entry(source, LogLevel::Warning, line);
    }

    fn append_error_log(&self, source: &str, line: String) {
        self.append_log_entry(source, LogLevel::Error, line);
    }

    fn action_scheduler_snapshot(&self) -> (Vec<QueuedActionState>, String, u32, u32, u32, bool) {
        let (
            queued_actions,
            active_action_kind,
            pending_downloads,
            pending_uploads,
            pending_scans,
            blocked,
        ) = {
            let scheduler = self
                .action_scheduler
                .0
                .lock()
                .expect("action scheduler poisoned");
            let pending_downloads = scheduler
                .queue
                .iter()
                .filter(|action| action.kind.counts_as_download())
                .count() as u32;
            let pending_uploads = scheduler
                .queue
                .iter()
                .filter(|action| action.kind.counts_as_upload())
                .count() as u32;
            let pending_scans = scheduler
                .queue
                .iter()
                .filter(|action| action.kind.counts_as_scan())
                .count() as u32;
            (
                scheduler
                    .queue
                    .iter()
                    .map(|action| action.kind.to_state())
                    .collect::<Vec<_>>(),
                scheduler.active_action_kind.clone(),
                pending_downloads,
                pending_uploads,
                pending_scans,
                scheduler.stop_after_current,
            )
        };
        (
            queued_actions,
            active_action_kind,
            pending_downloads,
            pending_uploads,
            pending_scans,
            blocked,
        )
    }

    fn sync_runtime_from_action_scheduler_blocking(&self) -> Result<()> {
        let (
            queued_actions,
            active_action_kind,
            pending_downloads,
            pending_uploads,
            pending_scans,
            blocked,
        ) = self.action_scheduler_snapshot();
        {
            let mut runtime = self.runtime_write_guard();
            let active_download = u32::from(active_action_kind == "hydrate");
            let active_upload = u32::from(active_action_kind == "upload");
            runtime.pending_downloads = pending_downloads + active_download;
            runtime.pending_uploads = pending_uploads + active_upload;
            runtime.queued_action_count = queued_actions.len() as u32;
            runtime.active_action_kind = active_action_kind.clone();
            runtime.sync_state = if runtime.sync_paused {
                SyncState::Paused
            } else if !active_action_kind.is_empty() || !queued_actions.is_empty() {
                if active_action_kind.starts_with("refresh")
                    || (pending_scans > 0
                        && runtime.pending_downloads == 0
                        && runtime.pending_uploads == 0)
                {
                    SyncState::Scanning
                } else {
                    SyncState::Syncing
                }
            } else if runtime.sync_state != SyncState::Error {
                SyncState::Idle
            } else {
                runtime.sync_state
            };
            if blocked && runtime.sync_paused {
                runtime.sync_state = SyncState::Paused;
            }
        }
        self.persist_runtime_blocking()?;
        self.emit_event(BackendEvent::SyncStateChanged);
        Ok(())
    }

    async fn persist_runtime(&self) -> Result<()> {
        let (queued_actions, active_action_kind, _, _, _, _) = self.action_scheduler_snapshot();
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
            active_action_kind,
            queued_actions,
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
            self.append_warning_log(
                "sync",
                format!("deferred upload for {relative_path} while sync is paused"),
            );
            return;
        }
        if let Err(error) = self.enqueue_background_action(ActionKind::Upload {
            path: relative_path.clone(),
        }) {
            self.set_path_error_sync(&relative_path, error.to_string());
        }
    }

    fn resume_upload_queue(self: &Arc<Self>) {
        {
            let mut scheduler = self
                .action_scheduler
                .0
                .lock()
                .expect("action scheduler poisoned");
            scheduler.stop_after_current = false;
            self.action_scheduler.1.notify_all();
        }
        let _ = self.sync_runtime_from_action_scheduler_blocking();
    }

    async fn pause_uploads_for_lifecycle(&self, clear_queue: bool) -> Result<()> {
        {
            let mut scheduler = self
                .action_scheduler
                .0
                .lock()
                .expect("action scheduler poisoned");
            scheduler.stop_after_current = true;
        }

        loop {
            let active_action_kind = {
                let scheduler = self
                    .action_scheduler
                    .0
                    .lock()
                    .expect("action scheduler poisoned");
                scheduler.active_action_kind.clone()
            };
            if active_action_kind != "upload"
                && active_action_kind != "refresh-tree"
                && active_action_kind != "hydrate"
                && active_action_kind != "evict"
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if clear_queue {
            let mut scheduler = self
                .action_scheduler
                .0
                .lock()
                .expect("action scheduler poisoned");
            let mut retained = VecDeque::with_capacity(scheduler.queue.len());
            while let Some(queued) = scheduler.queue.pop_front() {
                let cancel = matches!(
                    queued.kind,
                    ActionKind::Hydrate { .. }
                        | ActionKind::Evict { .. }
                        | ActionKind::Upload { .. }
                        | ActionKind::RefreshDirectory {
                            recursive: true,
                            ..
                        }
                );
                if cancel {
                    if let Some(responder) = queued.responder {
                        let _ = responder.send(Err(anyhow!(
                            "queued action cancelled for lifecycle transition"
                        )));
                    }
                } else {
                    retained.push_back(queued);
                }
            }
            scheduler.queue = retained;
        }

        self.sync_runtime_from_action_scheduler_blocking()?;
        Ok(())
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
        self.append_error_log("sync", message.clone());
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
        self.append_error_log("sync", message.clone());
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
            let mut state = current;
            state.hydrated = true;
            state.error.clear();
            state.state = derive_path_state(&state);
            state.last_sync_at = unix_timestamp();
            self.path_state_store.upsert_many(&[state])?;
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
            let status = self.run_rclone_status_blocking(
                binary.as_path(),
                build_download_args(&config, &self.paths, relative_path, &local_path),
                "failed to execute rclone copyto",
            )?;
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
        if !state.is_dir && (state.dirty || state.state == PathSyncState::Conflict) {
            bail!("cannot evict {} while it has local changes", relative_path);
        }
        let path = self.backing_file_path(relative_path)?;
        if path.exists() {
            if state.is_dir {
                fs::remove_dir_all(&path)
                    .with_context(|| format!("unable to remove {}", path.display()))?;
            } else {
                fs::remove_file(&path)
                    .with_context(|| format!("unable to remove {}", path.display()))?;
            }
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
            let remote_entry = self.fetch_remote_file_entry_sync(relative_path)?;
            match remote_entry {
                Some(remote_entry) => {
                    let revision = revision_for_entry(&remote_entry);
                    if revision != state.base_revision {
                        bail!("conflict detected for {}", relative_path);
                    }
                }
                None => {
                    bail!(
                        "conflict detected for {}: remote file disappeared",
                        relative_path
                    );
                }
            }
        }

        let config = self.config_read_guard().clone();
        let binary = resolve_rclone_binary(config.rclone_bin.as_deref())?;
        let _guard = self.rclone_process_lock.blocking_lock();
        let status = self.run_rclone_status_blocking(
            binary.as_path(),
            build_upload_args(&config, &self.paths, relative_path, &local_path),
            "failed to execute rclone copyto",
        )?;
        if !status.success() {
            bail!("rclone copyto upload failed for {}", relative_path);
        }

        let verified_remote = self
            .fetch_remote_file_entry_sync(relative_path)?
            .with_context(|| {
                format!("uploaded {relative_path}, but could not verify remote metadata")
            })?;

        state.dirty = false;
        state.error.clear();
        state.conflict_reason.clear();
        state.hydrated = true;
        state.size_bytes = verified_remote.size_bytes();
        state.last_sync_at =
            parse_rclone_mod_time_unix(&verified_remote.mod_time).unwrap_or_else(unix_timestamp);
        state.base_revision = revision_for_entry(&verified_remote);
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
        let status = self.run_rclone_status_blocking(
            binary.as_path(),
            build_mkdir_args(&config, &self.paths, relative_path),
            "failed to execute rclone mkdir",
        )?;
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
            let status = self.run_rclone_status_blocking(
                binary.as_path(),
                args,
                "failed to execute rclone delete",
            )?;
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
            let status = self.run_rclone_status_blocking(
                binary.as_path(),
                build_moveto_args(&config, &self.paths, from, to),
                "failed to execute rclone moveto",
            )?;
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

    fn run_rclone_status_blocking(
        &self,
        binary: &Path,
        args: Vec<OsString>,
        context: &str,
    ) -> Result<ExitStatus> {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.run_rclone_status_sync(binary, args, context))
        } else {
            self.run_rclone_status_sync(binary, args, context)
        }
    }

    fn run_rclone_output_blocking(
        &self,
        binary: &Path,
        args: Vec<OsString>,
        context: &str,
    ) -> Result<std::process::Output> {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.run_rclone_output_sync(binary, args, context))
        } else {
            self.run_rclone_output_sync(binary, args, context)
        }
    }

    fn read_rclone_version_blocking(&self, binary: PathBuf) -> Result<String> {
        let output = self.run_rclone_output_blocking(
            binary.as_path(),
            vec![OsString::from("version")],
            "failed to execute rclone version",
        )?;
        if !output.status.success() {
            bail!("rclone version exited with status {}", output.status);
        }
        String::from_utf8(output.stdout)
            .context("rclone version returned invalid utf-8")?
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .context("rclone version did not return any output")
    }

    fn run_rclone_status_sync(
        &self,
        binary: &Path,
        args: Vec<OsString>,
        context: &str,
    ) -> Result<ExitStatus> {
        let _guard = self.rclone_process_lock.blocking_lock();
        std::process::Command::new(binary)
            .args(args)
            .stdin(Stdio::null())
            .status()
            .with_context(|| context.to_string())
    }

    fn run_rclone_output_sync(
        &self,
        binary: &Path,
        args: Vec<OsString>,
        context: &str,
    ) -> Result<std::process::Output> {
        let _guard = self.rclone_process_lock.blocking_lock();
        std::process::Command::new(binary)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .with_context(|| context.to_string())
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

    fn ensure_directory(&self, path: &str) -> io::Result<()> {
        let backend = self.backend()?;
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| {
                backend.runtime_handle.block_on(
                    backend.ensure_directory_listing(if path.is_empty() {
                        None
                    } else {
                        Some(path)
                    }),
                )
            })
        } else {
            backend
                .runtime_handle
                .block_on(backend.ensure_directory_listing(if path.is_empty() {
                    None
                } else {
                    Some(path)
                }))
        }
        .map_err(io_error)
    }

    fn open_file(&self, path: &str, request: OpenRequest) -> io::Result<File> {
        let backend = self.backend()?;
        if request.create {
            backend
                .create_local_entry_sync(path, false)
                .map_err(io_error)?;
        } else {
            backend
                .run_queued_action_sync(ActionKind::Hydrate {
                    path: path.to_string(),
                })
                .map_err(io_error)?;
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
        backend
            .run_queued_action_sync(ActionKind::CreateDir {
                path: path.to_string(),
            })
            .map_err(io_error)?;
        backend
            .rebuild_path_state_snapshot_sync()
            .map_err(io_error)?;
        backend.emit_path_state_refresh(&[path.to_string()]);
        Ok(())
    }

    fn remove_file(&self, path: &str) -> io::Result<()> {
        let backend = self.backend()?;
        backend
            .run_queued_action_sync(ActionKind::RemovePath {
                path: path.to_string(),
                is_dir: false,
            })
            .map_err(io_error)?;
        backend
            .rebuild_path_state_snapshot_sync()
            .map_err(io_error)?;
        backend.emit_path_state_refresh(&[path.to_string()]);
        Ok(())
    }

    fn remove_dir(&self, path: &str) -> io::Result<()> {
        let backend = self.backend()?;
        backend
            .run_queued_action_sync(ActionKind::RemovePath {
                path: path.to_string(),
                is_dir: true,
            })
            .map_err(io_error)?;
        backend
            .rebuild_path_state_snapshot_sync()
            .map_err(io_error)?;
        backend.emit_path_state_refresh(&[path.to_string()]);
        Ok(())
    }

    fn rename_path(&self, from: &str, to: &str) -> io::Result<()> {
        let backend = self.backend()?;
        backend
            .run_queued_action_sync(ActionKind::RenamePath {
                from: from.to_string(),
                to: to.to_string(),
            })
            .map_err(io_error)?;
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

fn clear_remote_scan_error(message: &str) -> String {
    message
        .strip_prefix(REMOTE_SCAN_ERROR_PREFIX)
        .map_or_else(|| message.to_string(), |_| String::new())
}

fn mark_remote_scan_error(message: &str) -> String {
    if message.starts_with(REMOTE_SCAN_ERROR_PREFIX) {
        message.to_string()
    } else {
        format!("{REMOTE_SCAN_ERROR_PREFIX}{message}")
    }
}

fn expand_selected_paths(
    root_path: &Path,
    raw_paths: &[String],
    states: &[PathState],
) -> Result<BTreeSet<String>> {
    let state_map = states
        .iter()
        .map(|state| (state.path.clone(), state))
        .collect::<HashMap<_, _>>();
    let mut selected = BTreeSet::new();

    for raw_path in raw_paths {
        let relative = relative_string(&relative_path_for(root_path, Path::new(raw_path))?);
        let Some(state) = state_map.get(&relative) else {
            bail!("unknown path {relative}");
        };

        selected.insert(relative.clone());
        if state.is_dir {
            let prefix = format!("{relative}/");
            selected.extend(
                states
                    .iter()
                    .filter(|candidate| candidate.path.starts_with(&prefix))
                    .map(|candidate| candidate.path.clone()),
            );
        }
    }
    Ok(selected)
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

fn parse_mount_point_info(line: &str, path: &Path) -> Option<MountPointInfo> {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    let separator_index = fields.iter().position(|field| *field == "-")?;
    if fields.get(4).copied()? != path.to_string_lossy() {
        return None;
    }
    Some(MountPointInfo {
        fs_type: fields.get(separator_index + 1)?.to_string(),
        source: fields.get(separator_index + 2)?.to_string(),
    })
}

fn mount_point_info(path: &Path) -> Result<Option<MountPointInfo>> {
    let mountinfo = fs::read_to_string("/proc/self/mountinfo")
        .context("unable to inspect existing mount points")?;
    Ok(mountinfo
        .lines()
        .find_map(|line| parse_mount_point_info(line, path)))
}

fn is_openonedrive_mount(info: &MountPointInfo) -> bool {
    info.fs_type.starts_with("fuse") && info.source == OPENONEDRIVE_MOUNT_SOURCE
}

fn mountpoint_is_stale(path: &Path) -> bool {
    fs::read_dir(path).is_err_and(|error| {
        matches!(
            error.raw_os_error(),
            Some(libc::ENOTCONN) | Some(libc::EIO)
        )
    })
}

async fn unmount_mountpoint(path: &Path) -> Result<()> {
    let attempts = [
        ("fusermount", vec![OsStr::new("-u"), path.as_os_str()]),
        ("fusermount", vec![OsStr::new("-uz"), path.as_os_str()]),
        ("umount", vec![path.as_os_str()]),
        ("umount", vec![OsStr::new("-l"), path.as_os_str()]),
    ];
    let mut failures = Vec::new();

    for (program, args) in attempts {
        let output = match Command::new(program).args(&args).output().await {
            Ok(output) => output,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                failures.push(format!("{program}: {error}"));
                continue;
            }
        };
        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            output.status.to_string()
        };
        failures.push(format!("{program}: {detail}"));
    }

    bail!(
        "unable to clear stale filesystem mountpoint {} ({})",
        path.display(),
        failures.join("; ")
    )
}

fn create_dir_all_with_reason(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|error| anyhow!("unable to create {}: {error}", path.display()))
}

fn create_root_path_if_missing(path: &Path) -> Result<bool> {
    if path.exists() {
        if !path.is_dir() {
            bail!("root path must be a directory");
        }
        return Ok(false);
    }

    create_dir_all_with_reason(path)?;
    Ok(true)
}

fn normalize_directory_query_path(root_path: &Path, raw_path: &str) -> Result<Option<String>> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return Ok(None);
    }
    let relative = relative_path_for(root_path, Path::new(trimmed))?;
    Ok(Some(relative_string(&relative)))
}

fn sort_paths_for_eviction(
    paths: &BTreeSet<String>,
    state_map: &HashMap<String, &PathState>,
) -> Vec<String> {
    let mut ordered = paths.iter().cloned().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        let left_depth = left.matches('/').count();
        let right_depth = right.matches('/').count();
        let left_is_dir = state_map.get(left).is_some_and(|state| state.is_dir);
        let right_is_dir = state_map.get(right).is_some_and(|state| state.is_dir);

        right_depth
            .cmp(&left_depth)
            .then_with(|| left_is_dir.cmp(&right_is_dir))
            .then_with(|| left.cmp(right))
    });
    ordered
}

fn immediate_child_of(path: &str, parent: Option<&str>) -> bool {
    match parent {
        Some(parent) if !parent.is_empty() => path
            .strip_prefix(parent)
            .and_then(|remainder| remainder.strip_prefix('/'))
            .is_some_and(|remainder| !remainder.is_empty() && !remainder.contains('/')),
        _ => !path.contains('/'),
    }
}

fn path_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn path_matches_query(state: &PathState, query: &str) -> bool {
    let lower_path = state.path.to_lowercase();
    if lower_path.contains(query) {
        return true;
    }
    path_name(&state.path).to_lowercase().contains(query)
}

fn sort_path_states(states: &mut [PathState]) {
    states.sort_by(|left, right| {
        right
            .is_dir
            .cmp(&left.is_dir)
            .then_with(|| path_name(&left.path).cmp(path_name(&right.path)))
            .then_with(|| left.path.cmp(&right.path))
    });
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
    size: i64,
    #[serde(rename = "ModTime", default)]
    mod_time: String,
    #[serde(rename = "Hashes", default)]
    hashes: BTreeMap<String, String>,
}

impl RcloneListEntry {
    fn size_bytes(&self) -> u64 {
        u64::try_from(self.size).unwrap_or_default()
    }
}

fn revision_for_entry(entry: &RcloneListEntry) -> String {
    let hash_fragment = entry
        .hashes
        .iter()
        .find(|(_, value)| !value.is_empty())
        .map(|(name, value)| format!(":{name}={value}"))
        .unwrap_or_default();
    format!("{}:{}{}", entry.size_bytes(), entry.mod_time, hash_fragment)
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
                state.state = dominant_path_state(state.state.clone(), summary.state.clone());
                state.pinned |= summary.pinned;
                state.hydrated |= summary.hydrated;
                state.dirty |= summary.dirty;
                if state.error.is_empty() {
                    state.error = summary.error.clone();
                }
                if state.conflict_reason.is_empty() {
                    state.conflict_reason = summary.conflict_reason.clone();
                }
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

fn directory_metadata_for_listed_directories(
    listed_directories: &BTreeSet<String>,
) -> Vec<DirectoryMetadata> {
    let listed_at = unix_timestamp();
    let mut directories = listed_directories.clone();
    directories.insert(String::new());
    directories
        .into_iter()
        .map(|path| DirectoryMetadata {
            path,
            children_known: true,
            last_listed_at: listed_at,
        })
        .collect()
}

fn fully_listed_directories_for_entries(entries: &[RcloneListEntry]) -> BTreeSet<String> {
    let mut directories = BTreeSet::from([String::new()]);
    for entry in entries {
        if entry.path.is_empty() {
            continue;
        }
        if entry.is_dir {
            directories.insert(entry.path.clone());
        }
        let mut current = Path::new(&entry.path).parent();
        while let Some(parent) = current {
            if parent.as_os_str().is_empty() {
                break;
            }
            directories.insert(relative_string(parent));
            current = parent.parent();
        }
    }
    directories
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
    build_lsjson_directory_args(config, paths, None, true)
}

fn build_lsjson_directory_args(
    config: &AppConfig,
    paths: &ProjectPaths,
    relative_path: Option<&str>,
    recursive: bool,
) -> Vec<OsString> {
    let target = match relative_path.filter(|path| !path.is_empty()) {
        Some(path) => format!("{}:{path}", config.remote_name),
        None => format!("{}:", config.remote_name),
    };
    let mut args = vec![
        OsString::from("lsjson"),
        OsString::from(target),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
    ];
    if recursive {
        args.push(OsString::from("--recursive"));
    }
    args.push(OsString::from("--hash"));
    args.push(OsString::from("--no-mimetype"));
    args
}

fn build_lsjson_single_args(
    config: &AppConfig,
    paths: &ProjectPaths,
    relative_path: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("lsjson"),
        OsString::from(format!("{}:", config.remote_name)),
        OsString::from("--config"),
        paths.rclone_config_file.as_os_str().to_os_string(),
        OsString::from("--files-only"),
        OsString::from("--recursive"),
        OsString::from("--hash"),
        OsString::from("--no-mimetype"),
        OsString::from("--include"),
        OsString::from(format!("/{relative_path}")),
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

fn format_log_entry(entry: &LogEntry) -> String {
    let level = match entry.level {
        LogLevel::Info => "INFO",
        LogLevel::Warning => "WARN",
        LogLevel::Error => "ERROR",
    };
    format!(
        "[{}] [{}] [{}] {}",
        entry.timestamp_unix, level, entry.source, entry.message
    )
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

fn set_file_descriptor_inheritable(file: &File) -> Result<()> {
    use std::os::fd::AsRawFd;

    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error()).context("unable to inspect file descriptor flags");
    }

    let updated = flags & !libc::FD_CLOEXEC;
    if updated == flags {
        return Ok(());
    }

    let status = unsafe { libc::fcntl(fd, libc::F_SETFD, updated) };
    if status < 0 {
        return Err(io::Error::last_os_error())
            .context("unable to clear close-on-exec from file descriptor");
    }
    Ok(())
}

fn prefix_lsjson_entries(parent_path: &str, entries: Vec<RcloneListEntry>) -> Vec<RcloneListEntry> {
    entries
        .into_iter()
        .map(|mut entry| {
            let normalized = entry.path.trim_start_matches('/');
            if normalized.is_empty() {
                return entry;
            }

            entry.path = if normalized == parent_path
                || normalized
                    .strip_prefix(parent_path)
                    .is_some_and(|suffix| suffix.starts_with('/'))
            {
                normalized.to_string()
            } else {
                relative_string(&Path::new(parent_path).join(normalized))
            };
            entry
        })
        .collect()
}

fn apply_failed_remote_scan_directories(
    snapshot: &mut Vec<PathState>,
    failed_directories: &BTreeMap<String, String>,
) {
    for (path, error) in failed_directories {
        if let Some(state) = snapshot.iter_mut().find(|state| state.path == *path) {
            state.is_dir = true;
            state.state = PathSyncState::Error;
            state.error = error.clone();
            continue;
        }

        snapshot.push(PathState {
            path: path.clone(),
            is_dir: true,
            state: PathSyncState::Error,
            size_bytes: 0,
            pinned: false,
            hydrated: false,
            dirty: false,
            error: error.clone(),
            last_sync_at: unix_timestamp(),
            base_revision: String::new(),
            conflict_reason: String::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActionKind, ActionScheduler, BACKEND_NAME, QueuedAction, RcloneBackend, RcloneListEntry,
        Runtime, affected_relative_paths, apply_failed_remote_scan_directories,
        apply_filesystem_start_failure, build_connect_args, build_deletefile_args,
        build_download_args, build_lsjson_args, build_lsjson_directory_args,
        build_lsjson_single_args, build_moveto_args, build_rmdir_args, build_upload_args,
        clear_remote_scan_error, create_root_path_if_missing, derive_path_state,
        directory_metadata_for_listed_directories, expand_retry_paths, expand_selected_paths,
        fully_listed_directories_for_entries, immediate_child_of, is_legacy_onedrive_remote_error,
        is_openonedrive_mount, mark_remote_scan_error, normalize_path_state_snapshot,
        parse_mount_point_info, parse_rclone_mod_time_unix, path_matches_query,
        prefix_lsjson_entries, read_remote_config_section, relative_path_for,
        remote_config_needs_repair, resolve_rclone_binary_with_path, revision_for_entry,
        sort_path_states, MountPointInfo,
    };
    use openonedrive_config::{AppConfig, ProjectPaths};
    use openonedrive_ipc_types::{
        ConnectionState, FilesystemState, PathState, PathSyncState, SyncState,
    };
    use openonedrive_state::RuntimeState;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;
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
        let filtered_lsjson = build_lsjson_single_args(&config, &paths, "Docs/file.txt")
            .into_iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(filtered_lsjson.contains(&"--include".to_string()));
        assert!(filtered_lsjson.contains(&"/Docs/file.txt".to_string()));
        assert!(!build_deletefile_args(&config, &paths, "Docs/file.txt").is_empty());
        assert!(!build_rmdir_args(&config, &paths, "Docs").is_empty());
        assert!(!build_moveto_args(&config, &paths, "Docs/a.txt", "Docs/b.txt").is_empty());

        let nested_lsjson = build_lsjson_directory_args(&config, &paths, Some("Docs"), false)
            .into_iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(nested_lsjson.contains(&"openonedrive:Docs".to_string()));
        assert!(!nested_lsjson.contains(&"--recursive".to_string()));
    }

    #[test]
    fn lsjson_entry_prefixing_preserves_nested_paths() {
        let prefixed = prefix_lsjson_entries(
            "Docs",
            vec![
                RcloneListEntry {
                    path: "child.txt".into(),
                    is_dir: false,
                    size: 1,
                    mod_time: String::new(),
                    hashes: BTreeMap::new(),
                },
                RcloneListEntry {
                    path: "nested".into(),
                    is_dir: true,
                    size: 0,
                    mod_time: String::new(),
                    hashes: BTreeMap::new(),
                },
                RcloneListEntry {
                    path: "Docs/already-full.txt".into(),
                    is_dir: false,
                    size: 1,
                    mod_time: String::new(),
                    hashes: BTreeMap::new(),
                },
            ],
        );

        assert_eq!(
            prefixed
                .into_iter()
                .map(|entry| entry.path)
                .collect::<Vec<_>>(),
            vec![
                "Docs/child.txt".to_string(),
                "Docs/nested".to_string(),
                "Docs/already-full.txt".to_string()
            ]
        );
    }

    #[test]
    fn lsjson_directory_entries_accept_negative_size_markers() {
        let entries = serde_json::from_str::<Vec<RcloneListEntry>>(
            r#"[{"Path":"Docs","IsDir":true,"Size":-1,"ModTime":"2026-03-25T00:00:00Z","Hashes":{}}]"#,
        )
        .expect("parse lsjson");

        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].size_bytes(), 0);
        assert_eq!(revision_for_entry(&entries[0]), "0:2026-03-25T00:00:00Z");
    }

    #[test]
    fn immediate_child_detection_handles_root_and_nested_paths() {
        assert!(immediate_child_of("Docs", None));
        assert!(immediate_child_of("Docs/file.txt", Some("Docs")));
        assert!(!immediate_child_of("Docs/folder/file.txt", Some("Docs")));
        assert!(!immediate_child_of("Other/file.txt", Some("Docs")));
    }

    #[test]
    fn search_prefers_basename_and_path_matches() {
        let state = PathState {
            path: "Reports/Quarterly/report-final.xlsx".into(),
            is_dir: false,
            state: PathSyncState::OnlineOnly,
            size_bytes: 1,
            pinned: false,
            hydrated: false,
            dirty: false,
            error: String::new(),
            last_sync_at: 0,
            base_revision: String::new(),
            conflict_reason: String::new(),
        };
        assert!(path_matches_query(&state, "report-final"));
        assert!(path_matches_query(&state, "quarterly"));
        assert!(!path_matches_query(&state, "invoice"));
    }

    #[test]
    fn sorted_directory_states_show_directories_first() {
        let mut states = vec![
            PathState {
                path: "Docs/file.txt".into(),
                is_dir: false,
                state: PathSyncState::OnlineOnly,
                size_bytes: 1,
                pinned: false,
                hydrated: false,
                dirty: false,
                error: String::new(),
                last_sync_at: 0,
                base_revision: String::new(),
                conflict_reason: String::new(),
            },
            PathState {
                path: "Docs".into(),
                is_dir: true,
                state: PathSyncState::AvailableLocal,
                size_bytes: 0,
                pinned: false,
                hydrated: false,
                dirty: false,
                error: String::new(),
                last_sync_at: 0,
                base_revision: String::new(),
                conflict_reason: String::new(),
            },
        ];

        sort_path_states(&mut states);
        assert!(states[0].is_dir);
        assert_eq!(states[0].path, "Docs");
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

    #[tokio::test(flavor = "multi_thread")]
    async fn list_directory_returns_immediate_children_sorted() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        backend
            .path_state_store
            .replace_all(&[
                PathState {
                    path: "Docs".into(),
                    is_dir: true,
                    state: PathSyncState::AvailableLocal,
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
                    path: "Docs/alpha".into(),
                    is_dir: true,
                    state: PathSyncState::AvailableLocal,
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
                    path: "Docs/report.txt".into(),
                    is_dir: false,
                    state: PathSyncState::AvailableLocal,
                    size_bytes: 10,
                    pinned: false,
                    hydrated: true,
                    dirty: false,
                    error: String::new(),
                    last_sync_at: 1,
                    base_revision: "rev".into(),
                    conflict_reason: String::new(),
                },
                PathState {
                    path: "Vault".into(),
                    is_dir: true,
                    state: PathSyncState::Error,
                    size_bytes: 0,
                    pinned: false,
                    hydrated: false,
                    dirty: false,
                    error: "remote scan failed: invalidRequest".into(),
                    last_sync_at: 1,
                    base_revision: "dir".into(),
                    conflict_reason: String::new(),
                },
                PathState {
                    path: "Docs/alpha/spec.md".into(),
                    is_dir: false,
                    state: PathSyncState::PinnedLocal,
                    size_bytes: 12,
                    pinned: true,
                    hydrated: true,
                    dirty: false,
                    error: String::new(),
                    last_sync_at: 1,
                    base_revision: "rev".into(),
                    conflict_reason: String::new(),
                },
            ])
            .expect("path states");

        let root_entries = backend.list_directory("").await.expect("root entries");
        assert_eq!(
            root_entries
                .into_iter()
                .map(|state| state.path)
                .collect::<Vec<_>>(),
            vec!["Docs".to_string(), "Vault".to_string()]
        );

        let docs_entries = backend.list_directory("Docs").await.expect("docs entries");
        assert_eq!(
            docs_entries
                .into_iter()
                .map(|state| state.path)
                .collect::<Vec<_>>(),
            vec!["Docs/alpha".to_string(), "Docs/report.txt".to_string()]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn list_directory_refreshes_remote_root_without_hydrating_content() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend_with_mock_lsjson(
            dir.path(),
            r#"[
  {"Path":"Docs","IsDir":true,"Size":-1,"ModTime":"2026-03-26T00:00:00Z"},
  {"Path":"report.txt","IsDir":false,"Size":12,"ModTime":"2026-03-26T00:00:00Z","Hashes":{"sha1":"abc"}}
]"#,
        )
        .await;

        let entries = backend.list_directory("").await.expect("root entries");
        assert_eq!(
            entries
                .iter()
                .map(|state| state.path.as_str())
                .collect::<Vec<_>>(),
            vec!["Docs", "report.txt"]
        );
        assert!(entries.iter().all(|state| !state.hydrated));
        assert!(
            !dir.path()
                .join("OneDrive")
                .join(".openonedrive-cache")
                .join("report.txt")
                .exists()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn search_paths_matches_by_basename_and_full_path() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        backend
            .path_state_store
            .replace_all(&[
                PathState {
                    path: "Docs".into(),
                    is_dir: true,
                    state: PathSyncState::AvailableLocal,
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
                    path: "Docs/report.txt".into(),
                    is_dir: false,
                    state: PathSyncState::AvailableLocal,
                    size_bytes: 10,
                    pinned: false,
                    hydrated: true,
                    dirty: false,
                    error: String::new(),
                    last_sync_at: 1,
                    base_revision: "rev".into(),
                    conflict_reason: String::new(),
                },
                PathState {
                    path: "Pictures/report-cover.png".into(),
                    is_dir: false,
                    state: PathSyncState::OnlineOnly,
                    size_bytes: 11,
                    pinned: false,
                    hydrated: false,
                    dirty: false,
                    error: String::new(),
                    last_sync_at: 1,
                    base_revision: "rev".into(),
                    conflict_reason: String::new(),
                },
            ])
            .expect("path states");

        let results = backend.search_paths("report", 10).await.expect("search");
        assert_eq!(
            results
                .into_iter()
                .map(|state| state.path)
                .collect::<Vec<_>>(),
            vec![
                "Pictures/report-cover.png".to_string(),
                "Docs/report.txt".to_string()
            ]
        );
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
    fn expands_selected_directories_from_snapshot_without_visible_tree() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().join("root");
        fs::create_dir_all(&root).expect("root");
        let snapshot = vec![
            PathState {
                path: "docs".into(),
                is_dir: true,
                state: PathSyncState::OnlineOnly,
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
                path: "docs/nested".into(),
                is_dir: true,
                state: PathSyncState::OnlineOnly,
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
                path: "docs/readme.md".into(),
                is_dir: false,
                state: PathSyncState::OnlineOnly,
                size_bytes: 1,
                pinned: false,
                hydrated: false,
                dirty: false,
                error: String::new(),
                last_sync_at: 1,
                base_revision: "rev1".into(),
                conflict_reason: String::new(),
            },
            PathState {
                path: "docs/nested/spec.txt".into(),
                is_dir: false,
                state: PathSyncState::OnlineOnly,
                size_bytes: 1,
                pinned: false,
                hydrated: false,
                dirty: false,
                error: String::new(),
                last_sync_at: 1,
                base_revision: "rev2".into(),
                conflict_reason: String::new(),
            },
        ];

        let selected =
            expand_selected_paths(&root, &[root.join("docs").display().to_string()], &snapshot)
                .expect("expand selected paths");

        assert_eq!(
            selected.into_iter().collect::<Vec<_>>(),
            vec![
                "docs".to_string(),
                "docs/nested".to_string(),
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
            PathState {
                path: "Vault".into(),
                is_dir: true,
                state: PathSyncState::Error,
                size_bytes: 0,
                pinned: false,
                hydrated: false,
                dirty: false,
                error: "remote scan failed: invalidRequest".into(),
                last_sync_at: 3,
                base_revision: String::new(),
                conflict_reason: String::new(),
            },
        ]);

        assert_eq!(normalized.len(), 3);
        let docs = normalized
            .iter()
            .find(|state| state.path == "Docs")
            .expect("docs dir");
        assert_eq!(docs.state, PathSyncState::PinnedLocal);
        assert!(docs.pinned);

        let vault = normalized
            .into_iter()
            .find(|state| state.path == "Vault")
            .expect("vault dir");
        assert_eq!(vault.state, PathSyncState::Error);
        assert!(!vault.error.is_empty());
    }

    #[test]
    fn remote_scan_errors_clear_after_successful_listing() {
        assert_eq!(
            clear_remote_scan_error(&mark_remote_scan_error("boom")),
            String::new()
        );
        assert_eq!(clear_remote_scan_error("other failure"), "other failure");
    }

    #[test]
    fn listed_directory_metadata_only_marks_completed_directory_scans() {
        let metadata = directory_metadata_for_listed_directories(&BTreeSet::from([
            String::new(),
            "Docs".to_string(),
        ]));
        let paths = metadata
            .into_iter()
            .map(|entry| entry.path)
            .collect::<Vec<_>>();

        assert_eq!(paths, vec![String::new(), "Docs".to_string()]);
        assert!(!paths.contains(&"Docs/Sub".to_string()));
    }

    #[test]
    fn recursive_listing_marks_all_known_directories() {
        let directories = fully_listed_directories_for_entries(&[
            RcloneListEntry {
                path: "Docs".into(),
                is_dir: true,
                size: 0,
                mod_time: String::new(),
                hashes: BTreeMap::new(),
            },
            RcloneListEntry {
                path: "Docs/Sub/report.txt".into(),
                is_dir: false,
                size: 1,
                mod_time: String::new(),
                hashes: BTreeMap::new(),
            },
        ]);

        assert!(directories.contains(""));
        assert!(directories.contains("Docs"));
        assert!(directories.contains("Docs/Sub"));
    }

    #[test]
    fn failed_remote_scan_directories_mark_existing_directory_entries() {
        let mut snapshot = vec![PathState {
            path: "Vault".into(),
            is_dir: true,
            state: PathSyncState::OnlineOnly,
            size_bytes: 0,
            pinned: false,
            hydrated: false,
            dirty: false,
            error: String::new(),
            last_sync_at: 1,
            base_revision: "dir".into(),
            conflict_reason: String::new(),
        }];
        let failed = BTreeMap::from([(
            "Vault".to_string(),
            mark_remote_scan_error(
                "rclone lsjson failed for remote directory Vault: invalidRequest",
            ),
        )]);

        apply_failed_remote_scan_directories(&mut snapshot, &failed);

        let vault = snapshot
            .into_iter()
            .find(|state| state.path == "Vault")
            .expect("vault dir");
        assert_eq!(vault.state, PathSyncState::Error);
        assert!(vault.error.starts_with("remote scan failed: "));
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
    async fn complete_sync_activity_stays_syncing_while_transfers_remain() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        {
            let mut runtime = backend.runtime.write().await;
            runtime.pending_uploads = 1;
            runtime.sync_state = SyncState::Scanning;
        }

        backend
            .complete_sync_activity(None)
            .await
            .expect("complete sync activity");

        let status = backend.status().await.expect("status");
        assert_eq!(status.sync_state, SyncState::Syncing);
    }

    #[test]
    fn blocked_queue_prefers_foreground_actions_when_background_sync_is_stopped() {
        let mut scheduler = ActionScheduler {
            stop_after_current: true,
            ..ActionScheduler::default()
        };
        scheduler.queue.push_back(QueuedAction {
            kind: ActionKind::Upload {
                path: "Docs/report.txt".into(),
            },
            responder: None,
        });
        scheduler.queue.push_back(QueuedAction {
            kind: ActionKind::RefreshDirectory {
                path: None,
                recursive: true,
            },
            responder: None,
        });
        scheduler.queue.push_back(QueuedAction {
            kind: ActionKind::Hydrate {
                path: "Docs/spec.txt".into(),
            },
            responder: None,
        });

        assert_eq!(scheduler.next_runnable_action_index(), Some(2));
    }

    #[test]
    fn blocked_queue_waits_when_only_background_actions_remain() {
        let mut scheduler = ActionScheduler {
            stop_after_current: true,
            ..ActionScheduler::default()
        };
        scheduler.queue.push_back(QueuedAction {
            kind: ActionKind::Upload {
                path: "Docs/report.txt".into(),
            },
            responder: None,
        });
        scheduler.queue.push_back(QueuedAction {
            kind: ActionKind::RefreshDirectory {
                path: None,
                recursive: true,
            },
            responder: None,
        });

        assert_eq!(scheduler.next_runnable_action_index(), None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pause_sync_clears_queued_uploads_before_marking_paused() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        {
            let mut scheduler = backend
                .action_scheduler
                .0
                .lock()
                .expect("action scheduler poisoned");
            scheduler.queue.push_back(QueuedAction {
                kind: ActionKind::Upload {
                    path: "Docs/report.txt".into(),
                },
                responder: None,
            });
        }
        backend
            .sync_runtime_from_action_scheduler_blocking()
            .expect("sync runtime");

        backend.pause_sync().await.expect("pause sync");

        let scheduler = backend
            .action_scheduler
            .0
            .lock()
            .expect("action scheduler poisoned");
        assert!(scheduler.queue.is_empty());
        assert!(scheduler.stop_after_current);
        drop(scheduler);

        let status = backend.status().await.expect("status");
        assert_eq!(status.pending_uploads, 0);
        assert_eq!(status.sync_state, SyncState::Paused);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pause_lifecycle_waits_for_hydrate_and_cancels_queued_transfer_actions() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        let (hydrate_tx, hydrate_rx) = mpsc::channel();
        let (evict_tx, evict_rx) = mpsc::channel();
        let (upload_tx, upload_rx) = mpsc::channel();
        let (refresh_tx, refresh_rx) = mpsc::channel();
        {
            let mut scheduler = backend
                .action_scheduler
                .0
                .lock()
                .expect("action scheduler poisoned");
            scheduler.active_action_kind = "hydrate".into();
            scheduler.queue.push_back(QueuedAction {
                kind: ActionKind::Hydrate {
                    path: "Docs/keep.txt".into(),
                },
                responder: Some(hydrate_tx),
            });
            scheduler.queue.push_back(QueuedAction {
                kind: ActionKind::Evict {
                    path: "Docs/evict.txt".into(),
                },
                responder: Some(evict_tx),
            });
            scheduler.queue.push_back(QueuedAction {
                kind: ActionKind::Upload {
                    path: "Docs/upload.txt".into(),
                },
                responder: Some(upload_tx),
            });
            scheduler.queue.push_back(QueuedAction {
                kind: ActionKind::RefreshDirectory {
                    path: None,
                    recursive: true,
                },
                responder: Some(refresh_tx),
            });
        }

        let scheduler = backend.action_scheduler.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            let mut scheduler = scheduler.0.lock().expect("action scheduler poisoned");
            scheduler.active_action_kind.clear();
        });

        let started_at = std::time::Instant::now();
        backend
            .pause_uploads_for_lifecycle(true)
            .await
            .expect("pause lifecycle");

        assert!(started_at.elapsed() >= Duration::from_millis(50));
        let scheduler = backend
            .action_scheduler
            .0
            .lock()
            .expect("action scheduler poisoned");
        assert!(scheduler.queue.is_empty());
        assert!(scheduler.stop_after_current);
        drop(scheduler);

        let hydrate_error = hydrate_rx
            .recv()
            .expect("hydrate cancellation")
            .expect_err("hydrate should cancel");
        assert!(hydrate_error.to_string().contains("lifecycle transition"));
        let evict_error = evict_rx
            .recv()
            .expect("evict cancellation")
            .expect_err("evict should cancel");
        assert!(evict_error.to_string().contains("lifecycle transition"));
        let upload_error = upload_rx
            .recv()
            .expect("upload cancellation")
            .expect_err("upload should cancel");
        assert!(upload_error.to_string().contains("lifecycle transition"));
        let refresh_error = refresh_rx
            .recv()
            .expect("refresh cancellation")
            .expect_err("refresh should cancel");
        assert!(refresh_error.to_string().contains("lifecycle transition"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn prepare_remote_repair_clears_queued_uploads() {
        let dir = tempdir().expect("tempdir");
        let backend = build_backend(dir.path()).await;
        {
            let mut scheduler = backend
                .action_scheduler
                .0
                .lock()
                .expect("action scheduler poisoned");
            scheduler.queue.push_back(QueuedAction {
                kind: ActionKind::Upload {
                    path: "Docs/report.txt".into(),
                },
                responder: None,
            });
        }
        backend
            .sync_runtime_from_action_scheduler_blocking()
            .expect("sync runtime");

        backend
            .prepare_remote_repair()
            .await
            .expect("prepare repair");

        let scheduler = backend
            .action_scheduler
            .0
            .lock()
            .expect("action scheduler poisoned");
        assert!(scheduler.queue.is_empty());
        assert!(scheduler.stop_after_current);
        drop(scheduler);

        let status = backend.status().await.expect("status");
        assert_eq!(status.pending_uploads, 0);
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

    #[test]
    fn missing_root_path_is_created_when_requested() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().join("nested").join("OneDrive");

        assert!(!root.exists());
        let created = create_root_path_if_missing(&root).expect("create root path");

        assert!(created);
        assert!(root.is_dir());
        assert!(root.parent().expect("parent").is_dir());
    }

    #[test]
    fn parses_openonedrive_mountinfo_records() {
        let line = "284 24 0:143 / /dhdd/onedrive rw,nosuid,nodev,relatime - fuse openonedrive rw,user_id=1000,group_id=1000";
        let info = parse_mount_point_info(line, Path::new("/dhdd/onedrive"))
            .expect("mount info");

        assert_eq!(
            info,
            MountPointInfo {
                fs_type: "fuse".to_string(),
                source: "openonedrive".to_string(),
            }
        );
        assert!(is_openonedrive_mount(&info));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn backend_load_prepares_visible_root_folder() {
        let dir = tempdir().expect("tempdir");
        let paths = build_paths(dir.path());
        let mut config = AppConfig::default();
        config.root_path = dir.path().join("OneDrive");

        assert!(!config.root_path.exists());
        let _backend = RcloneBackend::load(paths, config.clone())
            .await
            .expect("backend");

        assert!(config.root_path.is_dir());
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

    async fn build_backend_with_mock_lsjson(
        root: &Path,
        lsjson_payload: &str,
    ) -> Arc<RcloneBackend> {
        let paths = build_paths(root);
        fs::create_dir_all(&paths.rclone_config_dir).expect("rclone config dir");
        fs::write(
            &paths.rclone_config_file,
            "[openonedrive]\ntype = onedrive\ndrive_id = drive\ndrive_type = business\n",
        )
        .expect("write config");

        let mock_rclone = root.join("mock-rclone.sh");
        fs::write(
            &mock_rclone,
            format!(
                "#!/usr/bin/env bash\nset -euo pipefail\nif [[ \"${{1:-}}\" == \"lsjson\" ]]; then\ncat <<'EOF'\n{lsjson_payload}\nEOF\n  exit 0\nfi\nif [[ \"${{1:-}}\" == \"version\" ]]; then\nprintf 'rclone v0-test\\n'\n  exit 0\nfi\nexit 0\n"
            ),
        )
        .expect("write mock rclone");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&mock_rclone).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&mock_rclone, permissions).expect("chmod");
        }

        let mut config = AppConfig::default();
        config.root_path = root.join("OneDrive");
        config.rclone_bin = Some(mock_rclone);
        fs::create_dir_all(&config.root_path).expect("root dir");
        RcloneBackend::load(paths, config).await.expect("backend")
    }
}
