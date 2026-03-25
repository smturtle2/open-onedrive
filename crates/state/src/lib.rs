use anyhow::{Context, Result};
use openonedrive_ipc_types::MountState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct RuntimeState {
    pub remote_configured: bool,
    pub mount_state: MountState,
    pub last_error: String,
    pub last_log_line: String,
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
        toml::from_str(&raw).with_context(|| format!("unable to parse {}", self.path.display()))
    }

    pub fn save(&self, state: &RuntimeState) -> Result<()> {
        let raw = toml::to_string_pretty(state).context("unable to serialize runtime state")?;
        fs::write(&self.path, raw)
            .with_context(|| format!("unable to write {}", self.path.display()))?;
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

#[cfg(test)]
mod tests {
    use super::{RuntimeState, StateStore};
    use openonedrive_ipc_types::MountState;
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
            mount_state: MountState::Mounted,
            last_error: "boom".into(),
            last_log_line: "mounted".into(),
        };

        store.save(&snapshot).expect("save");

        assert_eq!(store.load().expect("reload"), snapshot);
    }
}
