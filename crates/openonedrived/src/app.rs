use anyhow::{Context, Result};
use openonedrive_config::{AppConfig, ProjectPaths};
use openonedrive_ipc_types::StatusSnapshot;
use openonedrive_rclone_backend::RcloneBackend;
use std::fs;
use std::sync::Arc;

pub struct OpenOneDriveApp {
    backend: Arc<RcloneBackend>,
}

impl OpenOneDriveApp {
    pub async fn load() -> Result<Arc<Self>> {
        let paths = ProjectPaths::discover()?;
        paths.ensure()?;
        purge_legacy_state(&paths)?;
        let config = AppConfig::load_or_create(&paths)?;
        let backend = RcloneBackend::load(paths, config).await?;
        Ok(Arc::new(Self { backend }))
    }

    pub async fn begin_connect(self: &Arc<Self>) -> Result<()> {
        self.backend.begin_connect().await
    }

    pub async fn disconnect(self: &Arc<Self>) -> Result<()> {
        self.backend.disconnect().await
    }

    pub async fn set_mount_path(self: &Arc<Self>, path: &str) -> Result<()> {
        self.backend.set_mount_path(path).await
    }

    pub async fn mount(self: &Arc<Self>) -> Result<()> {
        self.backend.mount().await
    }

    pub async fn unmount(self: &Arc<Self>) -> Result<()> {
        self.backend.unmount().await
    }

    pub async fn retry_mount(self: &Arc<Self>) -> Result<()> {
        self.backend.retry_mount().await
    }

    pub async fn keep_local(self: &Arc<Self>, paths: &[String]) -> Result<u32> {
        self.backend.keep_local(paths).await
    }

    pub async fn make_online_only(self: &Arc<Self>, paths: &[String]) -> Result<u32> {
        self.backend.make_online_only(paths).await
    }

    pub async fn get_status(&self) -> Result<StatusSnapshot> {
        self.backend.status().await
    }

    pub async fn get_status_json(&self) -> Result<String> {
        serde_json::to_string(&self.get_status().await?).context("unable to serialize status")
    }

    pub async fn get_recent_log_lines(&self, limit: usize) -> Result<Vec<String>> {
        Ok(self.backend.recent_log_lines(limit).await)
    }
}

fn purge_legacy_state(paths: &ProjectPaths) -> Result<()> {
    remove_file_if_exists(&paths.legacy_db_file)?;
    remove_dir_if_exists(&paths.cache_dir.join("content"))?;
    Ok(())
}

fn remove_file_if_exists(path: &std::path::Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("unable to remove {}", path.display()))?;
    }
    Ok(())
}

fn remove_dir_if_exists(path: &std::path::Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("unable to remove {}", path.display()))?;
    }
    Ok(())
}
