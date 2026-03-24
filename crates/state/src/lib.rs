use anyhow::{Context, Result};
use openonedrive_ipc_types::{AvailabilityState, ItemKind, ItemSnapshot};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ManagedItem {
    pub path: String,
    pub kind: ItemKind,
    pub availability: AvailabilityState,
    pub pinned: bool,
    pub size: u64,
    pub modified_unix: i64,
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
        store.bootstrap_demo_content()?;
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
            "#,
        )?;
        Ok(())
    }

    fn bootstrap_demo_content(&self) -> Result<()> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        let has_items: Option<String> = connection
            .query_row("SELECT path FROM items LIMIT 1", [], |row| row.get(0))
            .optional()
            .context("unable to inspect initial item state")?;
        if has_items.is_some() {
            return Ok(());
        }

        let now = unix_time();
        let demo_items = [
            ManagedItem {
                path: "/Getting Started.txt".into(),
                kind: ItemKind::File,
                availability: AvailabilityState::OnlineOnly,
                pinned: false,
                size: 180,
                modified_unix: now,
                content_stub: Some(
                    "open-onedrive bootstrap item\n\nThis placeholder file exists so the initial FUSE mount has visible content while Graph sync is still being wired in.\n".into(),
                ),
            },
            ManagedItem {
                path: "/Documents".into(),
                kind: ItemKind::Directory,
                availability: AvailabilityState::Local,
                pinned: true,
                size: 0,
                modified_unix: now,
                content_stub: None,
            },
            ManagedItem {
                path: "/Documents/Placeholder Notes.txt".into(),
                kind: ItemKind::File,
                availability: AvailabilityState::Pinned,
                pinned: true,
                size: 96,
                modified_unix: now,
                content_stub: Some(
                    "Pinned items stay local.\n\nLater revisions will replace this stub with real OneDrive-backed content.\n".into(),
                ),
            },
        ];

        for item in demo_items {
            connection.execute(
                "INSERT INTO items (path, kind, availability, pinned, size, modified_unix, content_stub)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    item.path,
                    kind_to_db(item.kind),
                    availability_to_db(item.availability),
                    item.pinned as i64,
                    item.size as i64,
                    item.modified_unix,
                    item.content_stub,
                ],
            )?;
        }

        Ok(())
    }

    fn list_items_inner(&self, paths: Option<&[String]>) -> Result<Vec<ManagedItem>> {
        let connection = self.connection.lock().expect("sqlite mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT path, kind, availability, pinned, size, modified_unix, content_stub
             FROM items
             ORDER BY path ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ManagedItem {
                path: row.get(0)?,
                kind: db_to_kind(row.get::<_, String>(1)?.as_str()),
                availability: db_to_availability(row.get::<_, String>(2)?.as_str()),
                pinned: row.get::<_, i64>(3)? != 0,
                size: row.get::<_, i64>(4)? as u64,
                modified_unix: row.get(5)?,
                content_stub: row.get(6)?,
            })
        })?;

        let mut collected = Vec::new();
        for row in rows {
            let item = row?;
            if let Some(filter_paths) = paths {
                let normalised = normalize_path(&item.path);
                if !filter_paths
                    .iter()
                    .any(|path| normalize_path(path) == normalised)
                {
                    continue;
                }
            }
            collected.push(item);
        }

        Ok(collected)
    }
}

fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_secs() as i64
}

fn normalize_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
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
    use super::StateStore;
    use tempfile::tempdir;

    #[test]
    fn bootstraps_demo_items() {
        let dir = tempdir().expect("tempdir");
        let store = StateStore::open(&dir.path().join("state.sqlite3")).expect("state store");
        let items = store.list_items().expect("list items");
        assert!(!items.is_empty());
    }

    #[test]
    fn updates_pin_state() {
        let dir = tempdir().expect("tempdir");
        let store = StateStore::open(&dir.path().join("state.sqlite3")).expect("state store");
        store
            .set_availability(
                &[String::from("/Getting Started.txt")],
                openonedrive_ipc_types::AvailabilityState::Pinned,
                true,
            )
            .expect("pin item");
        let item = store
            .list_items_by_paths(&[String::from("/Getting Started.txt")])
            .expect("list item")
            .pop()
            .expect("one item");
        assert!(item.pinned);
    }
}

