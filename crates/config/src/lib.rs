use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub path_state_db_file: PathBuf,
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
            path_state_db_file: state_dir.join("path-state.sqlite3"),
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
    #[serde(alias = "mount_path")]
    pub root_path: PathBuf,
    pub remote_name: String,
    pub cache_limit_gb: u64,
    #[serde(alias = "auto_mount")]
    pub auto_start_filesystem: bool,
    pub custom_client_id: Option<String>,
    pub backing_dir_name: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            rclone_bin: None,
            root_path: Self::default_root_hint(),
            remote_name: Self::default_remote_name(),
            cache_limit_gb: 10,
            auto_start_filesystem: true,
            custom_client_id: None,
            backing_dir_name: Self::default_backing_dir_name(),
        }
    }
}

impl AppConfig {
    pub fn load(paths: &ProjectPaths) -> Result<Self> {
        if !paths.config_file.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&paths.config_file)
            .with_context(|| format!("unable to read {}", paths.config_file.display()))?;
        let config: Self = toml::from_str(&raw)
            .with_context(|| format!("unable to parse {}", paths.config_file.display()))?;
        Ok(config)
    }

    pub fn load_or_create(paths: &ProjectPaths) -> Result<Self> {
        paths.ensure()?;

        if !paths.config_file.exists() {
            let config = Self::default();
            config.save(paths)?;
            return Ok(config);
        }

        Self::load(paths)
    }

    pub fn save(&self, paths: &ProjectPaths) -> Result<()> {
        paths.ensure()?;
        let raw = toml::to_string_pretty(self).context("unable to serialize config")?;
        write_atomic(&paths.config_file, &raw)?;
        Ok(())
    }

    pub fn default_root_hint() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("OneDrive")
    }

    pub fn default_remote_name() -> String {
        "openonedrive".to_string()
    }

    pub fn default_backing_dir_name() -> String {
        ".openonedrive-cache".to_string()
    }

    pub fn backing_dir_path(&self) -> PathBuf {
        self.root_path.join(&self.backing_dir_name)
    }
}

pub fn validate_root_path(path: &Path, backing_dir_name: &str) -> Result<()> {
    if !path.is_absolute() {
        bail!("root path must be absolute");
    }
    if path == Path::new("/") {
        bail!("root path cannot be the filesystem root");
    }
    if is_known_mount_point(path)? {
        bail!("root path already exists as a mount point; stop that mount first");
    }

    if let Ok(metadata) = fs::metadata(path) {
        if !metadata.is_dir() {
            bail!("root path must be a directory");
        }
        let entries =
            fs::read_dir(path).with_context(|| format!("unable to inspect {}", path.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| format!("unable to inspect {}", path.display()))?;
            if entry.file_name().to_string_lossy() != backing_dir_name {
                bail!(
                    "root path must be empty except for the hidden backing directory {}",
                    backing_dir_name
                );
            }
        }
    } else {
        let nearest_existing_ancestor = path
            .ancestors()
            .find(|candidate| candidate.exists())
            .context("root path must have a writable parent directory")?;
        if !nearest_existing_ancestor.is_dir() {
            bail!("root path parent is not a directory");
        }
    }

    Ok(())
}

fn is_known_mount_point(path: &Path) -> Result<bool> {
    let mountinfo = fs::read_to_string("/proc/self/mountinfo")
        .context("unable to inspect existing mount points")?;

    Ok(mountinfo.lines().any(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        fields
            .get(4)
            .is_some_and(|mount_point| mount_point_matches(path, mount_point))
    }))
}

fn mount_point_matches(path: &Path, mount_point: &str) -> bool {
    let decoded_mount_point = decode_mountinfo_path(mount_point);
    path == decoded_mount_point
        || fs::canonicalize(path)
            .ok()
            .zip(fs::canonicalize(&decoded_mount_point).ok())
            .is_some_and(|(left, right)| left == right)
}

fn decode_mountinfo_path(raw: &str) -> PathBuf {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && index + 3 < bytes.len()
            && bytes[index + 1].is_ascii_digit()
            && bytes[index + 2].is_ascii_digit()
            && bytes[index + 3].is_ascii_digit()
        {
            let value = (bytes[index + 1] - b'0') * 64
                + (bytes[index + 2] - b'0') * 8
                + (bytes[index + 3] - b'0');
            decoded.push(value);
            index += 4;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    PathBuf::from(String::from_utf8_lossy(&decoded).into_owned())
}

fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let parent = path
        .parent()
        .context("target path must have a parent directory")?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("config.toml");
    let temp_path = parent.join(format!(".{file_name}.tmp-{stamp}"));
    fs::write(&temp_path, content)
        .with_context(|| format!("unable to write {}", temp_path.display()))?;
    fs::rename(&temp_path, path)
        .with_context(|| format!("unable to replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AppConfig, ProjectPaths, decode_mountinfo_path, mount_point_matches, validate_root_path,
    };
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    #[test]
    fn default_root_hint_is_absolute() {
        assert!(AppConfig::default_root_hint().is_absolute());
    }

    #[test]
    fn default_remote_name_matches_wrapper_remote() {
        assert_eq!(AppConfig::default_remote_name(), "openonedrive");
    }

    #[test]
    fn validates_empty_directory() {
        let dir = tempdir().expect("tempdir");
        validate_root_path(dir.path(), ".openonedrive-cache")
            .expect("empty directory should validate");
    }

    #[test]
    fn allows_hidden_backing_directory() {
        let dir = tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join(".openonedrive-cache")).expect("create cache dir");
        validate_root_path(dir.path(), ".openonedrive-cache")
            .expect("backing dir should be allowed");
    }

    #[test]
    fn rejects_non_empty_directory() {
        let dir = tempdir().expect("tempdir");
        std::fs::write(dir.path().join("occupied"), "busy").expect("write marker");
        assert!(validate_root_path(dir.path(), ".openonedrive-cache").is_err());
    }

    #[test]
    fn decodes_escaped_mountinfo_paths() {
        assert_eq!(
            decode_mountinfo_path("/tmp/One\\040Drive"),
            PathBuf::from("/tmp/One Drive")
        );
    }

    #[test]
    fn mount_point_matches_escaped_paths() {
        assert!(mount_point_matches(
            Path::new("/tmp/One Drive"),
            "/tmp/One\\040Drive"
        ));
    }

    #[test]
    fn allows_missing_nested_directory() {
        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("nested").join("OneDrive");
        validate_root_path(&target, ".openonedrive-cache")
            .expect("missing nested directory should validate");
    }

    #[test]
    fn discovers_paths() {
        let paths = ProjectPaths::discover().expect("discover xdg paths");
        assert!(paths.config_file.ends_with("config.toml"));
        assert!(paths.legacy_db_file.ends_with("state.sqlite3"));
        assert!(paths.path_state_db_file.ends_with("path-state.sqlite3"));
        assert!(paths.rclone_config_file.ends_with("rclone/rclone.conf"));
    }

    #[test]
    fn load_defaults_without_creating_files() {
        let dir = tempdir().expect("tempdir");
        let paths = ProjectPaths {
            config_dir: dir.path().join("config"),
            state_dir: dir.path().join("state"),
            cache_dir: dir.path().join("cache"),
            runtime_dir: dir.path().join("run"),
            config_file: dir.path().join("config").join("config.toml"),
            legacy_db_file: dir.path().join("state").join("state.sqlite3"),
            path_state_db_file: dir.path().join("state").join("path-state.sqlite3"),
            runtime_state_file: dir.path().join("state").join("runtime-state.toml"),
            rclone_config_dir: dir.path().join("config").join("rclone"),
            rclone_config_file: dir.path().join("config").join("rclone").join("rclone.conf"),
            rclone_cache_dir: dir.path().join("cache").join("rclone"),
        };

        let config = AppConfig::load(&paths).expect("load");
        assert_eq!(config, AppConfig::default());
        assert!(!paths.config_file.exists());
    }
}
