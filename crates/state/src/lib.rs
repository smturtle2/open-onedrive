use anyhow::{Context, Result};
use openonedrive_ipc_types::{AvailabilityState, ItemKind, ItemSnapshot};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ManagedItem {
    pub path: String,
    pub remote_id: Option<String>,
    pub parent_remote_id: Option<String>,
    pub name: String,
    pub kind: ItemKind,
    pub availability: AvailabilityState,
    pub pinned: bool,
    pub size: u64,
    pub modified_unix: i64,
    pub web_url: Option<String>,
    pub content_stub: Option<String>,
}

impl ManagedItem {
    pub fn to_snapshot(&self) -> ItemSnapshot {
        ItemSnapshot {
            path: self.path.clone(),
            kind: self.kind,
            availability: self.availability,
            pinned: self.pinned,
            syncing: false,
            has_error: self.availability == AvailabilityState::Error,
            size: self.size,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSession {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_unix: i64,
    pub scope: String,
    pub account_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCursor {
    pub delta_link: Option<String>,
    pub last_sync_unix: i64,
}

#[derive(Debug, Clone)]
pub struct RemoteItemRecord {
    pub path: String,
    pub remote_id: String,
    pub parent_remote_id: Option<String>,
    pub name: String,
    pub kind: ItemKind,
    pub size: u64,
    pub modified_unix: i64,
    pub web_url: Option<String>,
    pub content_stub: Option<String>,
}

pub struct StateStore {
    connection: Mutex<Connection>,
}

impl StateStore {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)
            .with_context(|| format!("unable to open sqlite database {}", path.display()))?;
        let store = Self {
            connection: Mutex::new(connection),
        };
        store.initialize_schema()?;
        Ok(store)
    }

    pub fn items_indexed(&self) -> Result<u64> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        let count: u64 = connection
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))
            .context("unable to count indexed items")?;
        Ok(count)
    }

    pub fn list_items(&self) -> Result<Vec<ManagedItem>> {
        self.list_items_inner(None)
    }

    pub fn list_items_by_paths(&self, paths: &[String]) -> Result<Vec<ManagedItem>> {
        if paths.is_empty() {
            return self.list_items();
        }
        self.list_items_inner(Some(paths))
    }

    pub fn set_availability(
        &self,
        paths: &[String],
        availability: AvailabilityState,
        pinned: bool,
    ) -> Result<()> {
        let mut connection = self.connection.lock().expect("sqlite mutex poisoned");
        let tx = connection.transaction().context("unable to create sqlite transaction")?;
        for path in paths {
            tx.execute(
                "UPDATE items SET availability = ?1, pinned = ?2 WHERE path = ?3",
                params![availability_to_db(availability), pinned as i64, normalize_path(path)],
            )
            .with_context(|| format!("unable to update state for {path}"))?;
        }
        tx.commit().context("unable to commit availability update")?;
        Ok(())
    }

    pub fn get_auth_session(&self) -> Result<Option<AuthSession>> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        connection
            .query_row(
                "SELECT client_id, access_token, refresh_token, expires_at_unix, scope, account_label
                 FROM auth_session
                 WHERE id = 1",
                [],
                |row| {
                    Ok(AuthSession {
                        client_id: row.get(0)?,
                        access_token: row.get(1)?,
                        refresh_token: row.get(2)?,
                        expires_at_unix: row.get(3)?,
                        scope: row.get(4)?,
                        account_label: row.get(5)?,
                    })
                },
            )
            .optional()
            .context("unable to load auth session")
    }

    pub fn set_auth_session(&self, session: &AuthSession) -> Result<()> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        connection
            .execute(
                "INSERT INTO auth_session (id, client_id, access_token, refresh_token, expires_at_unix, scope, account_label)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                   client_id = excluded.client_id,
                   access_token = excluded.access_token,
                   refresh_token = excluded.refresh_token,
                   expires_at_unix = excluded.expires_at_unix,
                   scope = excluded.scope,
                   account_label = excluded.account_label",
                params![
                    session.client_id,
                    session.access_token,
                    session.refresh_token,
                    session.expires_at_unix,
                    session.scope,
                    session.account_label,
                ],
            )
            .context("unable to persist auth session")?;
        Ok(())
    }

    pub fn clear_auth_session(&self) -> Result<()> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        connection
            .execute("DELETE FROM auth_session WHERE id = 1", [])
            .context("unable to clear auth session")?;
        Ok(())
    }

    pub fn get_sync_cursor(&self) -> Result<Option<SyncCursor>> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        connection
            .query_row(
                "SELECT delta_link, last_sync_unix FROM sync_cursor WHERE id = 1",
                [],
                |row| {
                    Ok(SyncCursor {
                        delta_link: row.get(0)?,
                        last_sync_unix: row.get(1)?,
                    })
                },
            )
            .optional()
            .context("unable to load sync cursor")
    }

    pub fn set_sync_cursor(&self, cursor: &SyncCursor) -> Result<()> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        connection
            .execute(
                "INSERT INTO sync_cursor (id, delta_link, last_sync_unix)
                 VALUES (1, ?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET
                   delta_link = excluded.delta_link,
                   last_sync_unix = excluded.last_sync_unix",
                params![cursor.delta_link, cursor.last_sync_unix],
            )
            .context("unable to persist sync cursor")?;
        Ok(())
    }

    pub fn clear_sync_cursor(&self) -> Result<()> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        connection
            .execute("DELETE FROM sync_cursor WHERE id = 1", [])
            .context("unable to clear sync cursor")?;
        Ok(())
    }

    pub fn clear_items(&self) -> Result<()> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        connection
            .execute("DELETE FROM items", [])
            .context("unable to clear indexed items")?;
        Ok(())
    }

    pub fn delete_items_by_remote_ids(&self, remote_ids: &[String]) -> Result<()> {
        if remote_ids.is_empty() {
            return Ok(());
        }

        let mut connection = self.connection.lock().expect("sqlite mutex poisoned");
        let tx = connection.transaction().context("unable to create sqlite transaction")?;
        for remote_id in remote_ids {
            tx.execute("DELETE FROM items WHERE remote_id = ?1", params![remote_id])
                .with_context(|| format!("unable to delete remote item {remote_id}"))?;
        }
        tx.commit().context("unable to commit remote deletions")?;
        Ok(())
    }

    pub fn upsert_remote_items(&self, items: &[RemoteItemRecord]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let mut connection = self.connection.lock().expect("sqlite mutex poisoned");
        let tx = connection.transaction().context("unable to create sqlite transaction")?;

        for item in items {
            let normalized_path = normalize_path(&item.path);
            let existing = tx
                .query_row(
                    "SELECT path, availability, pinned, content_stub
                     FROM items
                     WHERE remote_id = ?1 OR path = ?2
                     ORDER BY CASE WHEN remote_id = ?1 THEN 0 ELSE 1 END
                     LIMIT 1",
                    params![item.remote_id, normalized_path],
                    |row| {
                        Ok(ExistingItemState {
                            path: row.get(0)?,
                            availability: db_to_availability(row.get::<_, String>(1)?.as_str()),
                            pinned: row.get::<_, i64>(2)? != 0,
                            content_stub: row.get(3)?,
                        })
                    },
                )
                .optional()
                .with_context(|| format!("unable to load existing state for {}", item.remote_id))?;

            let (availability, pinned, content_stub) = existing
                .as_ref()
                .map(|value| (value.availability, value.pinned, value.content_stub.clone()))
                .unwrap_or_else(|| {
                    (
                        default_availability(item.kind),
                        false,
                        item.content_stub.clone(),
                    )
                });

            if let Some(existing) = existing.as_ref() {
                if existing.path != normalized_path {
                    tx.execute("DELETE FROM items WHERE path = ?1", params![existing.path])
                        .with_context(|| {
                            format!(
                                "unable to remove stale path {} before move",
                                existing.path
                            )
                        })?;
                }
            }

            tx.execute(
                "INSERT INTO items (
                    path,
                    remote_id,
                    parent_remote_id,
                    name,
                    kind,
                    availability,
                    pinned,
                    size,
                    modified_unix,
                    web_url,
                    content_stub
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(path) DO UPDATE SET
                    remote_id = excluded.remote_id,
                    parent_remote_id = excluded.parent_remote_id,
                    name = excluded.name,
                    kind = excluded.kind,
                    availability = excluded.availability,
                    pinned = excluded.pinned,
                    size = excluded.size,
                    modified_unix = excluded.modified_unix,
                    web_url = excluded.web_url,
                    content_stub = COALESCE(items.content_stub, excluded.content_stub)",
                params![
                    normalized_path,
                    item.remote_id,
                    item.parent_remote_id,
                    item.name,
                    kind_to_db(item.kind),
                    availability_to_db(availability),
                    pinned as i64,
                    saturating_u64_to_i64(item.size),
                    item.modified_unix,
                    item.web_url,
                    content_stub,
                ],
            )
            .with_context(|| format!("unable to upsert remote item {}", item.remote_id))?;
        }

        tx.commit().context("unable to commit remote item upsert")?;
        Ok(())
    }

    fn initialize_schema(&self) -> Result<()> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        connection.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;

            CREATE TABLE IF NOT EXISTS items (
              path TEXT PRIMARY KEY,
              kind TEXT NOT NULL,
              availability TEXT NOT NULL,
              pinned INTEGER NOT NULL DEFAULT 0,
              size INTEGER NOT NULL DEFAULT 0,
              modified_unix INTEGER NOT NULL,
              content_stub TEXT
            );

            CREATE TABLE IF NOT EXISTS operations (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              kind TEXT NOT NULL,
              target_path TEXT NOT NULL,
              created_unix INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS auth_session (
              id INTEGER PRIMARY KEY CHECK (id = 1),
              client_id TEXT NOT NULL,
              access_token TEXT NOT NULL,
              refresh_token TEXT,
              expires_at_unix INTEGER NOT NULL,
              scope TEXT NOT NULL,
              account_label TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS sync_cursor (
              id INTEGER PRIMARY KEY CHECK (id = 1),
              delta_link TEXT,
              last_sync_unix INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )?;

        ensure_column(&connection, "items", "remote_id", "TEXT")?;
        ensure_column(&connection, "items", "parent_remote_id", "TEXT")?;
        ensure_column(&connection, "items", "name", "TEXT NOT NULL DEFAULT ''")?;
        ensure_column(&connection, "items", "web_url", "TEXT")?;

        connection.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_items_remote_id
             ON items(remote_id)
             WHERE remote_id IS NOT NULL",
            [],
        )?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_items_parent_remote_id
             ON items(parent_remote_id)",
            [],
        )?;
        Ok(())
    }

    fn list_items_inner(&self, paths: Option<&[String]>) -> Result<Vec<ManagedItem>> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT path, remote_id, parent_remote_id, name, kind, availability, pinned, size, modified_unix, web_url, content_stub
             FROM items
             ORDER BY path ASC",
        )?;
        let rows = statement.query_map([], |row| {
            let path: String = row.get(0)?;
            Ok(ManagedItem {
                path: path.clone(),
                remote_id: row.get(1)?,
                parent_remote_id: row.get(2)?,
                name: row
                    .get::<_, String>(3)
                    .ok()
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| basename(&path)),
                kind: db_to_kind(row.get::<_, String>(4)?.as_str()),
                availability: db_to_availability(row.get::<_, String>(5)?.as_str()),
                pinned: row.get::<_, i64>(6)? != 0,
                size: row.get::<_, i64>(7)? as u64,
                modified_unix: row.get(8)?,
                web_url: row.get(9)?,
                content_stub: row.get(10)?,
            })
        })?;

        let mut collected = Vec::new();
        for row in rows {
            let item = row?;
            if let Some(filter_paths) = paths {
                let normalized = normalize_path(&item.path);
                if !filter_paths.iter().any(|path| normalize_path(path) == normalized) {
                    continue;
                }
            }
            collected.push(item);
        }

        Ok(collected)
    }
}

#[derive(Debug, Clone)]
struct ExistingItemState {
    path: String,
    availability: AvailabilityState,
    pinned: bool,
    content_stub: Option<String>,
}

fn ensure_column(connection: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut statement = connection
        .prepare(&pragma)
        .with_context(|| format!("unable to inspect schema for table {table}"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("unable to enumerate columns for table {table}"))?;

    if columns.iter().any(|value| value == column) {
        return Ok(());
    }

    let alter = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
    connection
        .execute(&alter, [])
        .with_context(|| format!("unable to add {column} to {table}"))?;
    Ok(())
}

#[cfg(test)]
fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_secs() as i64
}

fn normalize_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        "/".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn basename(path: &str) -> String {
    normalize_path(path)
        .trim_matches('/')
        .split('/')
        .next_back()
        .unwrap_or_default()
        .to_string()
}

fn default_availability(kind: ItemKind) -> AvailabilityState {
    match kind {
        ItemKind::Directory => AvailabilityState::Local,
        ItemKind::File => AvailabilityState::OnlineOnly,
    }
}

fn saturating_u64_to_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

fn availability_to_db(value: AvailabilityState) -> &'static str {
    match value {
        AvailabilityState::OnlineOnly => "online_only",
        AvailabilityState::Hydrating => "hydrating",
        AvailabilityState::Local => "local",
        AvailabilityState::Pinned => "pinned",
        AvailabilityState::Error => "error",
    }
}

fn db_to_availability(value: &str) -> AvailabilityState {
    match value {
        "online_only" => AvailabilityState::OnlineOnly,
        "hydrating" => AvailabilityState::Hydrating,
        "local" => AvailabilityState::Local,
        "pinned" => AvailabilityState::Pinned,
        "error" => AvailabilityState::Error,
        _ => AvailabilityState::Error,
    }
}

fn kind_to_db(value: ItemKind) -> &'static str {
    match value {
        ItemKind::File => "file",
        ItemKind::Directory => "directory",
    }
}

fn db_to_kind(value: &str) -> ItemKind {
    match value {
        "directory" => ItemKind::Directory,
        _ => ItemKind::File,
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthSession, RemoteItemRecord, StateStore, SyncCursor, unix_time};
    use openonedrive_ipc_types::{AvailabilityState, ItemKind};
    use tempfile::tempdir;

    #[test]
    fn new_store_starts_empty() {
        let dir = tempdir().expect("tempdir");
        let store = StateStore::open(&dir.path().join("state.sqlite3")).expect("state store");
        let items = store.list_items().expect("list items");
        assert!(items.is_empty());
    }

    #[test]
    fn persists_auth_session_round_trip() {
        let dir = tempdir().expect("tempdir");
        let store = StateStore::open(&dir.path().join("state.sqlite3")).expect("state store");
        let session = AuthSession {
            client_id: "client-id".into(),
            access_token: "access-token".into(),
            refresh_token: Some("refresh-token".into()),
            expires_at_unix: unix_time() + 3600,
            scope: "offline_access Files.ReadWrite.All User.Read".into(),
            account_label: "Ada Lovelace".into(),
        };

        store.set_auth_session(&session).expect("persist auth");

        assert_eq!(store.get_auth_session().expect("read auth"), Some(session));
    }

    #[test]
    fn persists_sync_cursor_round_trip() {
        let dir = tempdir().expect("tempdir");
        let store = StateStore::open(&dir.path().join("state.sqlite3")).expect("state store");
        let cursor = SyncCursor {
            delta_link: Some("https://graph.microsoft.com/v1.0/me/drive/root/delta?token=abc".into()),
            last_sync_unix: unix_time(),
        };

        store.set_sync_cursor(&cursor).expect("persist cursor");

        assert_eq!(store.get_sync_cursor().expect("read cursor"), Some(cursor));
    }

    #[test]
    fn upserts_and_deletes_remote_items() {
        let dir = tempdir().expect("tempdir");
        let store = StateStore::open(&dir.path().join("state.sqlite3")).expect("state store");

        store
            .upsert_remote_items(&[
                RemoteItemRecord {
                    path: "/Documents".into(),
                    remote_id: "folder-1".into(),
                    parent_remote_id: None,
                    name: "Documents".into(),
                    kind: ItemKind::Directory,
                    size: 0,
                    modified_unix: unix_time(),
                    web_url: Some("https://example.invalid/folder".into()),
                    content_stub: None,
                },
                RemoteItemRecord {
                    path: "/Documents/Notes.txt".into(),
                    remote_id: "file-1".into(),
                    parent_remote_id: Some("folder-1".into()),
                    name: "Notes.txt".into(),
                    kind: ItemKind::File,
                    size: 64,
                    modified_unix: unix_time(),
                    web_url: Some("https://example.invalid/file".into()),
                    content_stub: Some("placeholder".into()),
                },
            ])
            .expect("upsert remote items");

        store
            .set_availability(
                &[String::from("/Documents/Notes.txt")],
                AvailabilityState::Pinned,
                true,
            )
            .expect("pin item");

        store
            .upsert_remote_items(&[RemoteItemRecord {
                path: "/Documents/Notes Renamed.txt".into(),
                remote_id: "file-1".into(),
                parent_remote_id: Some("folder-1".into()),
                name: "Notes Renamed.txt".into(),
                kind: ItemKind::File,
                size: 72,
                modified_unix: unix_time(),
                web_url: Some("https://example.invalid/file-renamed".into()),
                content_stub: None,
            }])
            .expect("rename remote item");

        let renamed = store
            .list_items_by_paths(&[String::from("/Documents/Notes Renamed.txt")])
            .expect("list renamed");
        assert_eq!(renamed.len(), 1);
        assert!(renamed[0].pinned);
        assert_eq!(renamed[0].availability, AvailabilityState::Pinned);

        store
            .delete_items_by_remote_ids(&[String::from("file-1")])
            .expect("delete remote item");

        let items = store.list_items().expect("list items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].remote_id.as_deref(), Some("folder-1"));
    }
}
