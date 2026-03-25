use crate::mount::MountController;
use anyhow::{Context, Result, bail};
use openonedrive_auth::{
    build_authorization_request, exchange_authorization_code, redirect_uri,
    refresh_access_token,
};
use openonedrive_config::{AppConfig, ProjectPaths, validate_mount_path};
use openonedrive_graph::{DriveItem, GraphClient};
use openonedrive_ipc_types::{AvailabilityState, ItemKind, ItemSnapshot, MountState, StatusSnapshot, SyncState};
use openonedrive_state::{AuthSession, RemoteItemRecord, StateStore, SyncCursor};
use openonedrive_vfs::{SnapshotHandle, VirtualEntry};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use tracing::{info, warn};
use url::Url;

const CALLBACK_PORT: u16 = 53682;
const CALLBACK_BIND: &str = "127.0.0.1:53682";
const AUTH_SUCCESS_HTML: &str = r#"<!doctype html>
<html lang="en">
<meta charset="utf-8">
<title>open-onedrive</title>
<body style="font-family:sans-serif;padding:2rem;max-width:42rem;margin:0 auto;">
<h1>Authentication complete</h1>
<p>open-onedrive received the Microsoft callback. You can close this tab and return to the app.</p>
</body>
</html>"#;

pub struct OpenOneDriveApp {
    paths: ProjectPaths,
    config: RwLock<AppConfig>,
    state: StateStore,
    graph: GraphClient,
    mount: Mutex<MountController>,
    runtime: tokio::sync::RwLock<RuntimeStatus>,
    sync_lock: Mutex<()>,
}

#[derive(Debug, Clone)]
struct RuntimeStatus {
    sync_state: SyncState,
    mount_state: MountState,
    last_error: Option<String>,
    pending_login: Option<PendingLogin>,
}

#[derive(Debug, Clone)]
struct PendingLogin {
    client_id: String,
    csrf_state: String,
    pkce_verifier: String,
    redirect_uri: String,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            sync_state: SyncState::Starting,
            mount_state: MountState::Unmounted,
            last_error: None,
            pending_login: None,
        }
    }
}

impl OpenOneDriveApp {
    pub async fn load() -> Result<Arc<Self>> {
        let paths = ProjectPaths::discover()?;
        paths.ensure()?;
        let config = AppConfig::load_or_create(&paths)?;
        let state = StateStore::open(&paths.db_file)?;
        if state.get_auth_session()?.is_none() {
            state.clear_sync_cursor()?;
            state.clear_items()?;
        }
        let snapshot = SnapshotHandle::default();
        let app = Arc::new(Self {
            paths,
            config: RwLock::new(config),
            state,
            graph: GraphClient::default(),
            mount: Mutex::new(MountController::new(snapshot)),
            runtime: tokio::sync::RwLock::new(RuntimeStatus::default()),
            sync_lock: Mutex::new(()),
        });

        app.refresh_snapshot().await?;
        {
            let mut runtime = app.runtime.write().await;
            runtime.sync_state = SyncState::Idle;
        }
        app.spawn_background_tasks();
        if app.state.get_auth_session()?.is_some() {
            app.spawn_sync_task();
        }
        Ok(app)
    }

    pub fn config(&self) -> AppConfig {
        self.config.read().expect("config lock poisoned").clone()
    }

    pub async fn startup_mount(self: &Arc<Self>) -> Result<()> {
        if let Some(path) = self.config().mount_path {
            if !path.exists() {
                std::fs::create_dir_all(&path)
                    .with_context(|| format!("unable to create {}", path.display()))?;
            }
            self.mount_at(&path).await?;
        }
        Ok(())
    }

    pub async fn login(self: &Arc<Self>, client_id: &str) -> Result<String> {
        if client_id.trim().is_empty() {
            bail!("client ID cannot be empty");
        }

        let request = build_authorization_request(client_id, CALLBACK_PORT)?;
        {
            let mut config = self.config.write().expect("config lock poisoned");
            config.client_id = Some(client_id.trim().to_string());
            config.save(&self.paths)?;
        }
        {
            let mut runtime = self.runtime.write().await;
            runtime.last_error = None;
            runtime.pending_login = Some(PendingLogin {
                client_id: client_id.trim().to_string(),
                csrf_state: request.csrf_state,
                pkce_verifier: request.pkce_verifier,
                redirect_uri: request.redirect_uri,
            });
        }
        Ok(request.authorize_url)
    }

    pub async fn logout(self: &Arc<Self>) -> Result<()> {
        {
            let mut config = self.config.write().expect("config lock poisoned");
            config.client_id = None;
            config.save(&self.paths)?;
        }
        self.state.clear_auth_session()?;
        self.state.clear_sync_cursor()?;
        self.state.clear_items()?;
        self.refresh_snapshot().await?;
        let mut runtime = self.runtime.write().await;
        runtime.sync_state = SyncState::Idle;
        runtime.last_error = None;
        runtime.pending_login = None;
        Ok(())
    }

    pub async fn pause_sync(self: &Arc<Self>) {
        let mut runtime = self.runtime.write().await;
        runtime.sync_state = SyncState::Paused;
    }

    pub async fn resume_sync(self: &Arc<Self>) {
        {
            let mut runtime = self.runtime.write().await;
            runtime.sync_state = SyncState::Idle;
            runtime.last_error = None;
        }
        self.spawn_sync_task();
    }

    pub async fn set_mount_path(self: &Arc<Self>, raw_path: &str) -> Result<()> {
        let path = PathBuf::from(raw_path);
        validate_mount_path(&path)?;

        if !path.exists() {
            let parent = path.parent().context("mount path needs a parent directory")?;
            std::fs::create_dir_all(parent)
                .with_context(|| format!("unable to create {}", parent.display()))?;
            std::fs::create_dir_all(&path)
                .with_context(|| format!("unable to create {}", path.display()))?;
        }

        self.pause_sync().await;
        self.unmount().await;

        {
            let mut config = self.config.write().expect("config lock poisoned");
            config.mount_path = Some(path.clone());
            config.save(&self.paths)?;
        }

        self.mount_at(&path).await?;
        self.resume_sync().await;
        Ok(())
    }

    pub async fn pin(self: &Arc<Self>, paths: &[String]) -> Result<()> {
        let virtual_paths = self.normalize_virtual_paths(paths);
        self.state
            .set_availability(&virtual_paths, AvailabilityState::Pinned, true)?;
        self.refresh_snapshot().await?;
        Ok(())
    }

    pub async fn evict(self: &Arc<Self>, paths: &[String]) -> Result<()> {
        let virtual_paths = self.normalize_virtual_paths(paths);
        self.state
            .set_availability(&virtual_paths, AvailabilityState::OnlineOnly, false)?;
        self.refresh_snapshot().await?;
        Ok(())
    }

    pub async fn get_items(&self, paths: &[String]) -> Result<Vec<ItemSnapshot>> {
        let virtual_paths = self.normalize_virtual_paths(paths);
        let items = self.state.list_items_by_paths(&virtual_paths)?;
        Ok(items.into_iter().map(|item| item.to_snapshot()).collect())
    }

    pub async fn get_status(&self) -> Result<StatusSnapshot> {
        let config = self.config();
        let runtime = self.runtime.read().await.clone();
        Ok(StatusSnapshot {
            sync_state: runtime.sync_state,
            mount_state: runtime.mount_state,
            mount_path: config
                .mount_path
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            client_id_configured: config.client_id.is_some(),
            cache_limit_gb: config.cache_limit_gb,
            cache_usage_bytes: 0,
            items_indexed: self.state.items_indexed()?,
            last_error: runtime.last_error.unwrap_or_default(),
        })
    }

    pub async fn open_in_browser(&self, path: &str) -> Result<String> {
        let virtual_path = self
            .normalize_virtual_paths(&[path.to_string()])
            .into_iter()
            .next()
            .unwrap_or_else(|| "/".to_string());
        if virtual_path == "/" {
            return Ok(GraphClient::browser_url().to_string());
        }

        let item = self
            .state
            .list_items_by_paths(&[virtual_path])?
            .into_iter()
            .next();
        Ok(item
            .and_then(|entry| entry.web_url)
            .unwrap_or_else(|| GraphClient::browser_url().to_string()))
    }

    pub async fn retry_failed(self: &Arc<Self>) {
        {
            let mut runtime = self.runtime.write().await;
            runtime.last_error = None;
            if runtime.sync_state == SyncState::Error {
                runtime.sync_state = SyncState::Idle;
            }
        }
        self.spawn_sync_task();
    }

    fn spawn_background_tasks(self: &Arc<Self>) {
        let callback_app = self.clone();
        tokio::spawn(async move {
            if let Err(error) = callback_app.clone().run_callback_server().await {
                callback_app.record_error(error.to_string()).await;
            }
        });

        let polling_app = self.clone();
        tokio::spawn(async move {
            polling_app.run_polling_loop().await;
        });
    }

    fn spawn_sync_task(self: &Arc<Self>) {
        let app = self.clone();
        tokio::spawn(async move {
            if let Err(error) = app.sync_once().await {
                app.record_error(error.to_string()).await;
            }
        });
    }

    async fn run_callback_server(self: Arc<Self>) -> Result<()> {
        let listener = TcpListener::bind(CALLBACK_BIND)
            .await
            .with_context(|| format!("unable to bind OAuth callback listener on {CALLBACK_BIND}"))?;
        info!("OAuth callback listener ready on {CALLBACK_BIND}");

        loop {
            let (mut socket, _) = listener.accept().await.context("callback accept failed")?;
            let app = self.clone();
            tokio::spawn(async move {
                if let Err(error) = app.handle_callback_connection(&mut socket).await {
                    let _ = write_html_response(
                        &mut socket,
                        "500 Internal Server Error",
                        &render_error_html(&error.to_string()),
                    )
                    .await;
                    app.record_error(error.to_string()).await;
                }
            });
        }
    }

    async fn handle_callback_connection(
        self: &Arc<Self>,
        socket: &mut tokio::net::TcpStream,
    ) -> Result<()> {
        let mut buffer = vec![0_u8; 16 * 1024];
        let read = socket
            .read(&mut buffer)
            .await
            .context("unable to read callback request")?;
        if read == 0 {
            bail!("received empty callback request");
        }

        let request = String::from_utf8_lossy(&buffer[..read]);
        let target = parse_http_target(&request)?;
        let url = Url::parse(&format!("http://{CALLBACK_BIND}{target}"))
            .context("unable to parse callback URL")?;
        if url.path() != "/callback" {
            write_html_response(
                socket,
                "404 Not Found",
                &render_error_html("open-onedrive only accepts /callback."),
            )
            .await?;
            return Ok(());
        }

        let params = url.query_pairs().collect::<Vec<_>>();
        if let Some((_, error)) = params.iter().find(|(key, _)| key == "error") {
            let description = params
                .iter()
                .find(|(key, _)| key == "error_description")
                .map(|(_, value)| value.to_string())
                .unwrap_or_else(|| error.to_string());
            write_html_response(
                socket,
                "400 Bad Request",
                &render_error_html(&description),
            )
            .await?;
            bail!("authorization failed: {description}");
        }

        let code = params
            .iter()
            .find(|(key, _)| key == "code")
            .map(|(_, value)| value.to_string())
            .context("authorization callback did not include a code")?;
        let csrf_state = params
            .iter()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.to_string())
            .context("authorization callback did not include a state")?;
        let pending = {
            let runtime = self.runtime.read().await;
            runtime.pending_login.clone()
        }
        .context("no login is currently pending")?;

        if pending.csrf_state != csrf_state {
            write_html_response(
                socket,
                "400 Bad Request",
                &render_error_html("State mismatch during Microsoft callback."),
            )
            .await?;
            bail!("received mismatched OAuth state");
        }

        let token = exchange_authorization_code(
            &pending.client_id,
            &code,
            &pending.pkce_verifier,
            &pending.redirect_uri,
        )
        .await?;
        let profile = self.graph.current_user(&token.access_token).await.ok();
        let now = unix_time();
        let session = AuthSession {
            client_id: pending.client_id,
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_at_unix: now + token.expires_in_seconds.unwrap_or(3600) as i64,
            scope: token.scope,
            account_label: profile
                .map(|value| value.account_label())
                .unwrap_or_else(|| "Microsoft account".to_string()),
        };

        self.state.set_auth_session(&session)?;
        self.state.clear_sync_cursor()?;
        {
            let mut runtime = self.runtime.write().await;
            runtime.pending_login = None;
            runtime.last_error = None;
            if runtime.sync_state != SyncState::Paused {
                runtime.sync_state = SyncState::Idle;
            }
        }
        write_html_response(socket, "200 OK", AUTH_SUCCESS_HTML).await?;
        self.spawn_sync_task();
        Ok(())
    }

    async fn run_polling_loop(self: Arc<Self>) {
        loop {
            let delay = self.config().poll_min_sec.max(15);
            sleep(Duration::from_secs(delay)).await;

            let should_sync = {
                let runtime = self.runtime.read().await;
                runtime.sync_state != SyncState::Paused && runtime.pending_login.is_none()
            };
            if !should_sync {
                continue;
            }
            if self.state.get_auth_session().ok().flatten().is_none() {
                continue;
            }

            if let Err(error) = self.sync_once().await {
                self.record_error(error.to_string()).await;
            }
        }
    }

    async fn sync_once(self: &Arc<Self>) -> Result<()> {
        if self.state.get_auth_session()?.is_none() {
            return Ok(());
        }
        {
            let runtime = self.runtime.read().await;
            if runtime.sync_state == SyncState::Paused {
                return Ok(());
            }
        }

        let _guard = self.sync_lock.lock().await;
        {
            let mut runtime = self.runtime.write().await;
            if runtime.sync_state != SyncState::Paused {
                runtime.sync_state = SyncState::Polling;
                runtime.last_error = None;
            }
        }

        let result = self.sync_once_inner().await;
        match result {
            Ok(()) => {
                let mut runtime = self.runtime.write().await;
                if runtime.sync_state != SyncState::Paused {
                    runtime.sync_state = SyncState::Idle;
                }
                runtime.last_error = None;
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                let mut runtime = self.runtime.write().await;
                if runtime.sync_state != SyncState::Paused {
                    runtime.sync_state = SyncState::Error;
                }
                runtime.last_error = Some(message.clone());
                Err(error)
            }
        }
    }

    async fn sync_once_inner(&self) -> Result<()> {
        let session = self.ensure_fresh_session().await?;
        let existing_cursor = self.state.get_sync_cursor()?;
        let profile = match self.graph.current_user(&session.access_token).await {
            Ok(profile) => Some(profile),
            Err(error) => {
                warn!("unable to refresh Microsoft profile: {error:#}");
                None
            }
        };

        let delta = match self
            .graph
            .collect_drive_delta(
                &session.access_token,
                existing_cursor
                    .as_ref()
                    .and_then(|cursor| cursor.delta_link.as_deref()),
            )
            .await
        {
            Ok(delta) => delta,
            Err(error) if existing_cursor.as_ref().and_then(|cursor| cursor.delta_link.as_ref()).is_some() => {
                warn!("delta cursor rejected; falling back to a full resync: {error:#}");
                self.state.clear_sync_cursor()?;
                self.state.clear_items()?;
                self.graph.collect_drive_delta(&session.access_token, None).await?
            }
            Err(error) => return Err(error),
        };

        if existing_cursor
            .as_ref()
            .and_then(|cursor| cursor.delta_link.as_ref())
            .is_none()
        {
            self.state.clear_items()?;
        }

        let mut deleted_remote_ids = Vec::new();
        let mut upserts = Vec::new();
        for item in delta.items {
            if item.is_deleted() {
                if let Some(remote_id) = item.id {
                    deleted_remote_ids.push(remote_id);
                }
                continue;
            }

            if let Some(mapped) = map_drive_item(&item) {
                upserts.push(mapped);
            }
        }

        self.state.delete_items_by_remote_ids(&deleted_remote_ids)?;
        self.state.upsert_remote_items(&upserts)?;
        self.state.set_sync_cursor(&SyncCursor {
            delta_link: delta.delta_link,
            last_sync_unix: unix_time(),
        })?;
        let account_label = profile
            .map(|value| value.account_label())
            .unwrap_or_else(|| session.account_label.clone());
        self.state.set_auth_session(&AuthSession {
            account_label,
            ..session
        })?;
        self.refresh_snapshot().await?;
        Ok(())
    }

    async fn ensure_fresh_session(&self) -> Result<AuthSession> {
        let session = self
            .state
            .get_auth_session()?
            .context("Microsoft account is not connected")?;
        if session.expires_at_unix > unix_time() + 60 {
            return Ok(session);
        }

        let refresh_token = session
            .refresh_token
            .clone()
            .context("access token expired and no refresh token is available")?;
        let client_id = self
            .config()
            .client_id
            .unwrap_or_else(|| session.client_id.clone());
        let refreshed =
            refresh_access_token(&client_id, &refresh_token, &redirect_uri(CALLBACK_PORT)).await?;
        let updated = AuthSession {
            client_id,
            access_token: refreshed.access_token,
            refresh_token: refreshed.refresh_token.or(Some(refresh_token)),
            expires_at_unix: unix_time() + refreshed.expires_in_seconds.unwrap_or(3600) as i64,
            scope: refreshed.scope,
            account_label: session.account_label,
        };
        self.state.set_auth_session(&updated)?;
        Ok(updated)
    }

    async fn refresh_snapshot(&self) -> Result<()> {
        let items = self.state.list_items()?;
        let entries: Vec<VirtualEntry> = items
            .into_iter()
            .map(|item| VirtualEntry {
                path: item.path,
                kind: item.kind,
                availability: item.availability,
                pinned: item.pinned,
                size: item.size,
                modified_unix: item.modified_unix,
                content_stub: item.content_stub,
            })
            .collect();
        let mount = self.mount.lock().await;
        mount.rebuild(&entries);
        Ok(())
    }

    async fn record_error(&self, message: String) {
        let mut runtime = self.runtime.write().await;
        if runtime.sync_state != SyncState::Paused {
            runtime.sync_state = SyncState::Error;
        }
        runtime.last_error = Some(message);
    }

    fn normalize_virtual_paths(&self, paths: &[String]) -> Vec<String> {
        let mount_root = self.config().mount_path.unwrap_or_default();

        paths.iter()
            .map(|path| {
                let candidate = PathBuf::from(path);
                if mount_root.as_os_str().is_empty() || !candidate.is_absolute() {
                    return normalize_virtual_path_string(path);
                }

                if let Ok(stripped) = candidate.strip_prefix(&mount_root) {
                    let stripped = stripped.display().to_string();
                    if stripped.is_empty() {
                        "/".to_string()
                    } else {
                        normalize_virtual_path_string(&stripped)
                    }
                } else {
                    normalize_virtual_path_string(path)
                }
            })
            .collect()
    }

    async fn mount_at(&self, path: &Path) -> Result<()> {
        {
            let mut runtime = self.runtime.write().await;
            runtime.mount_state = MountState::Mounting;
            runtime.last_error = None;
        }

        let result = async {
            let mut mount = self.mount.lock().await;
            mount.mount(path)
        }
        .await;

        match result {
            Ok(()) => {
                let mut runtime = self.runtime.write().await;
                runtime.mount_state = MountState::Mounted;
                Ok(())
            }
            Err(error) => {
                let mut runtime = self.runtime.write().await;
                runtime.mount_state = MountState::Error;
                runtime.last_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    async fn unmount(&self) {
        let mut mount = self.mount.lock().await;
        mount.unmount();
        let mut runtime = self.runtime.write().await;
        runtime.mount_state = MountState::Unmounted;
    }
}

fn map_drive_item(item: &DriveItem) -> Option<RemoteItemRecord> {
    let path = item.normalized_path()?;
    if path == "/" {
        return None;
    }

    let remote_id = item.id.clone()?;
    let name = item.name.clone()?;
    let kind = if item.is_directory() {
        ItemKind::Directory
    } else {
        ItemKind::File
    };

    Some(RemoteItemRecord {
        path: path.clone(),
        remote_id,
        parent_remote_id: item.parent_remote_id().map(ToOwned::to_owned),
        name,
        kind,
        size: item.size.unwrap_or(0),
        modified_unix: item
            .last_modified_date_time
            .as_deref()
            .and_then(parse_graph_timestamp)
            .unwrap_or_else(unix_time),
        web_url: item.web_url.clone(),
        content_stub: (kind == ItemKind::File).then(|| build_content_stub(item, &path)),
    })
}

fn build_content_stub(item: &DriveItem, path: &str) -> String {
    let kind = if item.is_directory() { "directory" } else { "file" };
    let web_url = item
        .web_url
        .as_deref()
        .unwrap_or("https://onedrive.live.com/");
    format!(
        "open-onedrive cloud placeholder\n\npath: {path}\nkind: {kind}\nsize: {}\nsource: {web_url}\n",
        item.size.unwrap_or(0)
    )
}

fn parse_graph_timestamp(value: &str) -> Option<i64> {
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|date_time| date_time.unix_timestamp())
}

async fn write_html_response(
    socket: &mut tokio::net::TcpStream,
    status: &str,
    body: &str,
) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    );
    socket
        .write_all(response.as_bytes())
        .await
        .context("unable to write callback response")
}

fn parse_http_target(request: &str) -> Result<&str> {
    let request_line = request.lines().next().context("missing HTTP request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().context("missing HTTP method")?;
    if method != "GET" {
        bail!("callback server only supports GET requests");
    }
    parts.next().context("missing HTTP request target")
}

fn render_error_html(message: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>open-onedrive</title><body style=\"font-family:sans-serif;padding:2rem;max-width:42rem;margin:0 auto;\"><h1>Authentication failed</h1><p>{message}</p></body></html>"
    )
}

fn normalize_virtual_path_string(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "/".to_string();
    }
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn unix_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_secs() as i64
}
