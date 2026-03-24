use serde::{Deserialize, Serialize};
use zvariant::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum SyncState {
    Starting,
    Idle,
    Polling,
    Paused,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum MountState {
    Unmounted,
    Mounting,
    Mounted,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum AvailabilityState {
    OnlineOnly,
    Hydrating,
    Local,
    Pinned,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum ItemKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub sync_state: SyncState,
    pub mount_state: MountState,
    pub mount_path: String,
    pub client_id_configured: bool,
    pub cache_limit_gb: u64,
    pub cache_usage_bytes: u64,
    pub items_indexed: u64,
    pub last_error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct ItemSnapshot {
    pub path: String,
    pub kind: ItemKind,
    pub availability: AvailabilityState,
    pub pinned: bool,
    pub syncing: bool,
    pub has_error: bool,
    pub size: u64,
}
