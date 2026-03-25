use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const QUALIFIER: &str = "io.github";
const ORGANIZATION: &str = "smturtle2";
const APPLICATION: &str = "open-onedrive";

#[derive(Debug, Clone)]
pub struct ProjectPaths {
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub config_file: PathBuf,
    pub legacy_db_file: PathBuf,
    pub runtime_state_file: PathBuf,
    pub rclone_config_dir: PathBuf,
    pub rclone_config_file: PathBuf,
    pub rclone_cache_dir: PathBuf,
}

impl ProjectPaths {
    pub fn discover() -> Result<Self> {
        let project_dirs = ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .context("unable to resolve XDG project directories")?;
        let config_dir = project_dirs.config_dir().to_path_buf();
        let state_dir = project_dirs
            .state_dir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| project_dirs.data_local_dir().join("state"));
        let cache_dir = project_dirs.cache_dir().to_path_buf();
        let runtime_dir = project_dirs
            .runtime_dir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| state_dir.join("run"));
        let rclone_config_dir = config_dir.join("rclone");
        let rclone_cache_dir = cache_dir.join("rclone");

        Ok(Self {
            config_file: config_dir.join("config.toml"),
            legacy_db_file: state_dir.join("state.sqlite3"),
            runtime_state_file: state_dir.join("runtime-state.toml"),
            rclone_config_file: rclone_config_dir.join("rclone.conf"),
            config_dir,
            state_dir,
            cache_dir,
            runtime_dir,
            rclone_config_dir,
            rclone_cache_dir,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        for dir in [
            &self.config_dir,
            &self.state_dir,
            &self.cache_dir,
            &self.runtime_dir,
            &self.rclone_config_dir,
            &self.rclone_cache_dir,
        ] {
            fs::create_dir_all(dir)
                .with_context(|| format!("unable to create directory {}", dir.display()))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppConfig {
    pub rclone_bin: Option<PathBuf>,
    pub mount_path: PathBuf,
    pub remote_name: String,
    pub cache_limit_gb: u64,
    pub auto_mount: bool,
    pub custom_client_id: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            rclone_bin: None,
            mount_path: Self::default_mount_hint(),
            remote_name: Self::default_remote_name(),
            cache_limit_gb: 10,
            auto_mount: true,
            custom_client_id: None,
        }
    }
}

impl AppConfig {
    pub fn load_or_create(paths: &ProjectPaths) -> Result<Self> {
        paths.ensure()?;

        if !paths.config_file.exists() {
            let config = Self::default();
            config.save(paths)?;
            return Ok(config);
        }

        let raw = fs::read_to_string(&paths.config_file)
            .with_context(|| format!("unable to read {}", paths.config_file.display()))?;
        let config: Self = toml::from_str(&raw)
            .with_context(|| format!("unable to parse {}", paths.config_file.display()))?;
        Ok(config)
    }

    pub fn save(&self, paths: &ProjectPaths) -> Result<()> {
        paths.ensure()?;
        let raw = toml::to_string_pretty(self).context("unable to serialize config")?;
        fs::write(&paths.config_file, raw)
            .with_context(|| format!("unable to write {}", paths.config_file.display()))?;
        Ok(())
    }

    pub fn default_mount_hint() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("OneDrive")
    }

    pub fn default_remote_name() -> String {
        "openonedrive".to_string()
    }
}

pub fn validate_mount_path(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("mount path must be absolute");
    }
    if path == Path::new("/") {
        bail!("mount path cannot be the filesystem root");
    }
    if is_known_mount_point(path)? {
        bail!("mount path already exists as a mount point");
    }

    if let Ok(metadata) = fs::metadata(path) {
        if !metadata.is_dir() {
            bail!("mount path must be a directory");
        }
        let mut entries = fs::read_dir(path)
            .with_context(|| format!("unable to inspect {}", path.display()))?;
        if entries.next().is_some() {
            bail!("mount path must be empty");
        }
    } else {
        let parent = path
            .parent()
            .context("mount path must have a writable parent directory")?;
        if !parent.exists() {
            bail!("mount path parent directory does not exist");
        }
        if !parent.is_dir() {
            bail!("mount path parent is not a directory");
        }
    }

    Ok(())
}

fn is_known_mount_point(path: &Path) -> Result<bool> {
    let mountinfo = fs::read_to_string("/proc/self/mountinfo")
        .context("unable to inspect existing mount points")?;
    let canonical = path.to_string_lossy();

    Ok(mountinfo.lines().any(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        fields.get(4).copied() == Some(canonical.as_ref())
    }))
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, ProjectPaths, validate_mount_path};
    use tempfile::tempdir;

    #[test]
    fn default_mount_hint_is_absolute() {
        assert!(AppConfig::default_mount_hint().is_absolute());
    }

    #[test]
    fn default_remote_name_matches_wrapper_remote() {
        assert_eq!(AppConfig::default_remote_name(), "openonedrive");
    }

    #[test]
    fn validates_empty_directory() {
        let dir = tempdir().expect("tempdir");
        validate_mount_path(dir.path()).expect("empty directory should validate");
    }

    #[test]
    fn rejects_non_empty_directory() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(dir.path().join("occupied"), "busy").expect("write marker");
        assert!(validate_mount_path(dir.path()).is_err());
    }

    #[test]
    fn discovers_paths() {
        let paths = ProjectPaths::discover().expect("discover xdg paths");
        assert!(paths.config_file.ends_with("config.toml"));
        assert!(paths.legacy_db_file.ends_with("state.sqlite3"));
        assert!(paths.rclone_config_file.ends_with("rclone/rclone.conf"));
    }
}
