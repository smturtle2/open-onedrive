use serde::{Deserialize, Serialize};
use zvariant::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
pub enum MountState {
    #[default]
    Disconnected,
    Connecting,
    Mounting,
    Mounted,
    Unmounted,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, Default)]
pub struct StatusSnapshot {
    pub backend: String,
    pub remote_configured: bool,
    pub mount_state: MountState,
    pub mount_path: String,
    pub cache_usage_bytes: u64,
    pub rclone_version: String,
    pub last_error: String,
    pub last_log_line: String,
    pub custom_client_id_configured: bool,
}
