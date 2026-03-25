use anyhow::{Context, Result, bail};
use openonedrive_config::{AppConfig, ProjectPaths, validate_mount_path};
use openonedrive_ipc_types::{MountState, StatusSnapshot};
use openonedrive_state::{RuntimeState, StateStore};
use std::collections::VecDeque;
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
use tokio::sync::{Mutex, RwLock};
use tracing::warn;

const MAX_RECENT_LOGS: usize = 200;
const DEFAULT_DIR_CACHE_TIME: &str = "5m";
const DEFAULT_POLL_INTERVAL: &str = "1m";
pub const BACKEND_NAME: &str = "rclone";

#[derive(Debug, Clone)]
struct Runtime {
    remote_configured: bool,
    mount_state: MountState,
    last_error: String,
    last_log_line: String,
    rclone_version: String,
    mount_desired: bool,
    restart_attempt: u32,
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
            last_error: state.last_error,
            last_log_line: state.last_log_line,
            rclone_version: String::new(),
            mount_desired: false,
            restart_attempt: 0,
        }
    }
}

pub struct RcloneBackend {
    paths: ProjectPaths,
    config: RwLock<AppConfig>,
    state_store: StateStore,
    runtime: RwLock<Runtime>,
    recent_logs: Mutex<VecDeque<String>>,
    connect_child: Mutex<Option<Child>>,
    mount_child: Mutex<Option<Child>>,
    connect_generation: Mutex<u64>,
    mount_generation: Mutex<u64>,
}

impl RcloneBackend {
    pub async fn load(paths: ProjectPaths, config: AppConfig) -> Result<Arc<Self>> {
        paths.ensure()?;
        let state_store = StateStore::open(&paths.runtime_state_file)?;
        let persisted = state_store.load()?;
        let remote_configured = has_remote_config(&paths.rclone_config_file, &config.remote_name)?;

        let backend = Arc::new(Self {
            paths,
            config: RwLock::new(config),
            state_store,
            runtime: RwLock::new(Runtime::from_state(persisted, remote_configured)),
            recent_logs: Mutex::new(VecDeque::with_capacity(MAX_RECENT_LOGS)),
            connect_child: Mutex::new(None),
            mount_child: Mutex::new(None),
            connect_generation: Mutex::new(0),
            mount_generation: Mutex::new(0),
        });

        backend.refresh_rclone_version().await;

        if backend.current_config().await.auto_mount && remote_configured {
            if let Err(error) = backend.mount().await {
                backend.record_error(error.to_string()).await;
            }
        }

        Ok(backend)
    }

    pub async fn current_config(&self) -> AppConfig {
        self.config.read().await.clone()
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
                        match has_remote_config(
                            &backend.paths.rclone_config_file,
                            &config.remote_name,
                        ) {
                            Ok(true) => {
                                backend.set_remote_configured(true).await;
                                if config.auto_mount {
                                    if let Err(error) = backend.mount().await {
                                        backend.record_error(error.to_string()).await;
                                    }
                                } else {
                                    backend.set_mount_state(MountState::Unmounted).await;
                                }
                            }
                            Ok(false) => {
                                backend
                                    .record_error(
                                        "rclone finished without writing the app-owned remote"
                                            .to_string(),
                                    )
                                    .await;
                            }
                            Err(error) => {
                                backend.record_error(error.to_string()).await;
                            }
                        }
                    }
                    Ok(status) => {
                        backend
                            .record_error(format!(
                                "rclone config create exited with status {status}"
                            ))
                            .await;
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
            runtime.mount_desired = false;
            runtime.restart_attempt = 0;
            runtime.last_error.clear();
            runtime.last_log_line.clear();
        }
        self.recent_logs.lock().await.clear();
        self.persist_runtime().await?;
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
        self.set_mount_state(MountState::Mounted).await;

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

    pub async fn status(&self) -> Result<StatusSnapshot> {
        if self.runtime.read().await.rclone_version.is_empty() {
            self.refresh_rclone_version().await;
        }

        let runtime = self.runtime.read().await.clone();
        let config = self.current_config().await;
        Ok(StatusSnapshot {
            backend: BACKEND_NAME.to_string(),
            remote_configured: runtime.remote_configured,
            mount_state: runtime.mount_state,
            mount_path: config.mount_path.display().to_string(),
            cache_usage_bytes: directory_size_bytes(&self.paths.rclone_cache_dir)?,
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

    async fn refresh_rclone_version(&self) {
        let config = self.current_config().await;
        let version = resolve_rclone_binary(config.rclone_bin.as_deref())
            .and_then(read_rclone_version)
            .unwrap_or_default();
        let mut runtime = self.runtime.write().await;
        runtime.rclone_version = version;
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
    }

    async fn persist_runtime(&self) -> Result<()> {
        let runtime = self.runtime.read().await;
        self.state_store.save(&RuntimeState {
            remote_configured: runtime.remote_configured,
            mount_state: runtime.mount_state,
            last_error: runtime.last_error.clone(),
            last_log_line: runtime.last_log_line.clone(),
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
        OsString::from("personal"),
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("[{now}]")
}

#[cfg(test)]
mod tests {
    use super::{
        RcloneBackend, build_mount_args, resolve_rclone_binary_with_path, restart_backoff,
    };
    use openonedrive_config::{AppConfig, ProjectPaths};
    use openonedrive_ipc_types::MountState;
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
    fn restart_backoff_caps_exponentially() {
        assert_eq!(restart_backoff(1), Duration::from_secs(2));
        assert_eq!(restart_backoff(2), Duration::from_secs(4));
        assert_eq!(restart_backoff(5), Duration::from_secs(32));
        assert_eq!(restart_backoff(8), Duration::from_secs(32));
    }

    #[tokio::test]
    async fn fake_rclone_can_connect_mount_and_disconnect() {
        let dir = tempdir().expect("tempdir");
        let fake_rclone = dir.path().join("fake-rclone");
        let mount_path = dir.path().join("mount");
        fs::create_dir_all(&mount_path).expect("mount dir");
        write_fake_rclone(&fake_rclone);

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

    fn build_paths(root: &Path) -> ProjectPaths {
        ProjectPaths {
            config_dir: root.join("config"),
            state_dir: root.join("state"),
            cache_dir: root.join("cache"),
            runtime_dir: root.join("run"),
            config_file: root.join("config").join("config.toml"),
            legacy_db_file: root.join("state").join("state.sqlite3"),
            runtime_state_file: root.join("state").join("runtime-state.toml"),
            rclone_config_dir: root.join("config").join("rclone"),
            rclone_config_file: root.join("config").join("rclone").join("rclone.conf"),
            rclone_cache_dir: root.join("cache").join("rclone"),
        }
    }

    fn write_fake_rclone(path: &Path) {
        let script = r#"#!/bin/sh
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
  exit 0
fi

if [ "$cmd" = "mount" ]; then
  remote="$1"
  mount_path="$2"
  echo "mounted $remote at $mount_path"
  trap 'exit 0' TERM INT
  while true; do
    sleep 1
  done
fi

echo "unexpected invocation" >&2
exit 1
"#;

        fs::write(path, script).expect("write fake rclone");
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod");
    }
}
