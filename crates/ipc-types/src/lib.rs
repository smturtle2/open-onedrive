use serde::{Deserialize, Serialize};
use zvariant::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Ready,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
pub enum FilesystemState {
    #[default]
    Stopped,
    Starting,
    Running,
    Degraded,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
pub enum SyncState {
    #[default]
    Idle,
    Scanning,
    Syncing,
    Paused,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
pub enum LogLevel {
    #[default]
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
pub enum PathSyncState {
    #[default]
    OnlineOnly,
    AvailableLocal,
    PinnedLocal,
    Syncing,
    Conflict,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
pub struct PathState {
    pub path: String,
    pub is_dir: bool,
    pub state: PathSyncState,
    pub size_bytes: u64,
    pub pinned: bool,
    pub hydrated: bool,
    pub dirty: bool,
    pub error: String,
    pub last_sync_at: u64,
    pub base_revision: String,
    pub conflict_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
pub struct LogEntry {
    pub timestamp_unix: u64,
    pub source: String,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
pub struct StatusSnapshot {
    pub backend: String,
    pub remote_configured: bool,
    pub needs_remote_repair: bool,
    pub connection_state: ConnectionState,
    pub filesystem_state: FilesystemState,
    pub sync_state: SyncState,
    pub root_path: String,
    pub backing_dir_name: String,
    pub backing_usage_bytes: u64,
    pub pinned_file_count: u32,
    pub pending_downloads: u32,
    pub pending_uploads: u32,
    pub conflict_count: u32,
    pub last_sync_at: u64,
    pub last_sync_error: String,
    pub rclone_version: String,
    pub last_error: String,
    pub last_log_line: String,
    pub custom_client_id_configured: bool,
}
