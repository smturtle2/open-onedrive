use anyhow::{Context, Result};
use openonedrive_ipc_types::{ConnectionState, FilesystemState, SyncState};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct RuntimeState {
    pub remote_configured: bool,
    pub connection_state: ConnectionState,
    pub filesystem_state: FilesystemState,
    pub sync_state: SyncState,
    pub last_error: String,
    pub last_sync_error: String,
    pub last_log_line: String,
    pub pinned_relative_paths: Vec<String>,
    pub pending_downloads: u32,
    pub pending_uploads: u32,
    pub conflict_relative_paths: Vec<String>,
    pub last_sync_at: u64,
    pub sync_paused: bool,
}

pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("unable to create {}", parent.display()))?;
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    pub fn load(&self) -> Result<RuntimeState> {
        if !self.path.exists() {
            return Ok(RuntimeState::default());
        }

        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("unable to read {}", self.path.display()))?;
        match toml::from_str(&raw) {
            Ok(state) => Ok(state),
            Err(error) => {
                backup_corrupt_file(&self.path);
                eprintln!(
                    "warning: ignoring corrupt runtime state at {}: {error}",
                    self.path.display()
                );
                Ok(RuntimeState::default())
            }
        }
    }

    pub fn save(&self, state: &RuntimeState) -> Result<()> {
        let raw = toml::to_string_pretty(state).context("unable to serialize runtime state")?;
        write_atomic(&self.path, &raw)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)
                .with_context(|| format!("unable to remove {}", self.path.display()))?;
        }
        Ok(())
    }
}

fn backup_corrupt_file(path: &Path) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("runtime-state.toml");
    let backup_path = path.with_file_name(format!("{file_name}.corrupt-{stamp}"));
    let _ = fs::rename(path, &backup_path);
}

fn write_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create {}", parent.display()))?;
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("runtime-state.toml");
    let temp_path = path.with_file_name(format!("{file_name}.tmp-{stamp}"));
    fs::write(&temp_path, content)
        .with_context(|| format!("unable to write {}", temp_path.display()))?;
    fs::rename(&temp_path, path)
        .with_context(|| format!("unable to replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{RuntimeState, StateStore};
    use openonedrive_ipc_types::{ConnectionState, FilesystemState, SyncState};
    use tempfile::tempdir;

    #[test]
    fn missing_state_file_defaults_cleanly() {
        let dir = tempdir().expect("tempdir");
        let store = StateStore::open(&dir.path().join("runtime-state.toml")).expect("store");
        assert_eq!(store.load().expect("load"), RuntimeState::default());
    }

    #[test]
    fn runtime_state_round_trips() {
        let dir = tempdir().expect("tempdir");
        let store = StateStore::open(&dir.path().join("runtime-state.toml")).expect("store");
        let snapshot = RuntimeState {
            remote_configured: true,
            connection_state: ConnectionState::Ready,
            filesystem_state: FilesystemState::Running,
            sync_state: SyncState::Syncing,
            last_error: "boom".into(),
            last_sync_error: "sync boom".into(),
            last_log_line: "filesystem running".into(),
            pinned_relative_paths: vec!["dir/hello.txt".into()],
            pending_downloads: 2,
            pending_uploads: 1,
            conflict_relative_paths: vec!["dir/conflict.txt".into()],
            last_sync_at: 123,
            sync_paused: true,
        };

        store.save(&snapshot).expect("save");

        assert_eq!(store.load().expect("reload"), snapshot);
    }

    #[test]
    fn corrupt_state_is_ignored_and_backed_up() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("runtime-state.toml");
        std::fs::write(&path, "not = [valid").expect("write corrupt state");
        let store = StateStore::open(&path).expect("store");

        assert_eq!(store.load().expect("load"), RuntimeState::default());
        assert!(!path.exists());
        assert!(
            dir.path()
                .read_dir()
                .expect("read dir")
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
        );
    }
}
