use anyhow::{Context, Result};
use openonedrive_ipc_types::{PathState, PathSyncState};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct PathStateStore {
    db_path: PathBuf,
}

impl PathStateStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("unable to create {}", parent.display()))?;
        }

        let store = Self {
            db_path: path.to_path_buf(),
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn replace_all(&self, states: &[PathState]) -> Result<()> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .context("path-state transaction failed")?;
        transaction
            .execute("DELETE FROM path_states", [])
            .context("unable to clear path states")?;
        for state in states {
            upsert_state(&transaction, state)?;
        }
        transaction
            .execute(
                "INSERT INTO metadata(key, value) VALUES ('last_sync_at', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [unix_timestamp().to_string()],
            )
            .context("unable to persist last_sync_at")?;
        transaction
            .commit()
            .context("unable to commit path state snapshot")?;
        Ok(())
    }

    pub fn upsert_many(&self, states: &[PathState]) -> Result<()> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .context("path-state transaction failed")?;
        for state in states {
            upsert_state(&transaction, state)?;
        }
        transaction
            .commit()
            .context("unable to commit path state update")?;
        Ok(())
    }

    pub fn get_many(&self, paths: &[String]) -> Result<Vec<PathState>> {
        let connection = self.connection()?;
        let mut states = Vec::with_capacity(paths.len());
        for path in paths {
            if let Some(state) = load_state(&connection, path)? {
                states.push(state);
            }
        }
        Ok(states)
    }

    pub fn all(&self) -> Result<Vec<PathState>> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT path, is_dir, state, size_bytes, pinned, hydrated, dirty, error,
                        last_sync_at, base_revision, conflict_reason
                 FROM path_states
                 ORDER BY path ASC",
            )
            .context("unable to prepare path state listing")?;
        let rows = statement
            .query_map([], |row| {
                Ok(PathState {
                    path: row.get(0)?,
                    is_dir: row.get::<_, i64>(1)? != 0,
                    state: parse_state(&row.get::<_, String>(2)?),
                    size_bytes: row.get(3)?,
                    pinned: row.get::<_, i64>(4)? != 0,
                    hydrated: row.get::<_, i64>(5)? != 0,
                    dirty: row.get::<_, i64>(6)? != 0,
                    error: row.get(7)?,
                    last_sync_at: row.get(8)?,
                    base_revision: row.get(9)?,
                    conflict_reason: row.get(10)?,
                })
            })
            .context("unable to query path states")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("unable to load path states")
    }

    pub fn clear(&self) -> Result<()> {
        let connection = self.connection()?;
        connection
            .execute("DELETE FROM path_states", [])
            .context("unable to clear path states")?;
        Ok(())
    }

    fn initialize(&self) -> Result<()> {
        let connection = self.connection()?;
        connection
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS path_states (
                    path TEXT PRIMARY KEY NOT NULL,
                    is_dir INTEGER NOT NULL,
                    state TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL DEFAULT 0,
                    pinned INTEGER NOT NULL DEFAULT 0,
                    hydrated INTEGER NOT NULL DEFAULT 0,
                    dirty INTEGER NOT NULL DEFAULT 0,
                    error TEXT NOT NULL DEFAULT '',
                    last_sync_at INTEGER NOT NULL DEFAULT 0,
                    base_revision TEXT NOT NULL DEFAULT '',
                    conflict_reason TEXT NOT NULL DEFAULT ''
                );
                CREATE TABLE IF NOT EXISTS metadata (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL
                );
                ",
            )
            .context("unable to initialize path-state database")?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection> {
        Connection::open(&self.db_path)
            .with_context(|| format!("unable to open {}", self.db_path.display()))
    }
}

fn upsert_state(connection: &Connection, state: &PathState) -> Result<()> {
    connection
        .execute(
            "INSERT INTO path_states(
                 path, is_dir, state, size_bytes, pinned, hydrated, dirty, error,
                 last_sync_at, base_revision, conflict_reason
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(path) DO UPDATE SET
                 is_dir = excluded.is_dir,
                 state = excluded.state,
                 size_bytes = excluded.size_bytes,
                 pinned = excluded.pinned,
                 hydrated = excluded.hydrated,
                 dirty = excluded.dirty,
                 error = excluded.error,
                 last_sync_at = excluded.last_sync_at,
                 base_revision = excluded.base_revision,
                 conflict_reason = excluded.conflict_reason",
            params![
                state.path,
                state.is_dir as i64,
                state_name(&state.state),
                state.size_bytes,
                state.pinned as i64,
                state.hydrated as i64,
                state.dirty as i64,
                state.error,
                state.last_sync_at,
                state.base_revision,
                state.conflict_reason,
            ],
        )
        .with_context(|| format!("unable to upsert path state {}", state.path))?;
    Ok(())
}

fn load_state(connection: &Connection, path: &str) -> Result<Option<PathState>> {
    connection
        .query_row(
            "SELECT path, is_dir, state, size_bytes, pinned, hydrated, dirty, error,
                    last_sync_at, base_revision, conflict_reason
             FROM path_states
             WHERE path = ?1",
            [path],
            |row| {
                Ok(PathState {
                    path: row.get(0)?,
                    is_dir: row.get::<_, i64>(1)? != 0,
                    state: parse_state(&row.get::<_, String>(2)?),
                    size_bytes: row.get(3)?,
                    pinned: row.get::<_, i64>(4)? != 0,
                    hydrated: row.get::<_, i64>(5)? != 0,
                    dirty: row.get::<_, i64>(6)? != 0,
                    error: row.get(7)?,
                    last_sync_at: row.get(8)?,
                    base_revision: row.get(9)?,
                    conflict_reason: row.get(10)?,
                })
            },
        )
        .optional()
        .with_context(|| format!("unable to query path state {path}"))
}

fn state_name(state: &PathSyncState) -> &'static str {
    match state {
        PathSyncState::OnlineOnly => "online_only",
        PathSyncState::AvailableLocal => "available_local",
        PathSyncState::PinnedLocal => "pinned_local",
        PathSyncState::Syncing => "syncing",
        PathSyncState::Conflict => "conflict",
        PathSyncState::Error => "error",
    }
}

fn parse_state(value: &str) -> PathSyncState {
    match value {
        "available_local" => PathSyncState::AvailableLocal,
        "pinned_local" => PathSyncState::PinnedLocal,
        "syncing" => PathSyncState::Syncing,
        "conflict" => PathSyncState::Conflict,
        "error" => PathSyncState::Error,
        _ => PathSyncState::OnlineOnly,
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::PathStateStore;
    use openonedrive_ipc_types::{PathState, PathSyncState};
    use tempfile::tempdir;

    #[test]
    fn path_state_snapshot_round_trips() {
        let dir = tempdir().expect("tempdir");
        let store = PathStateStore::open(&dir.path().join("path-state.sqlite3")).expect("store");
        let snapshot = vec![
            PathState {
                path: "Broken Vault".into(),
                is_dir: true,
                state: PathSyncState::Error,
                size_bytes: 0,
                pinned: false,
                hydrated: false,
                dirty: false,
                error: "remote scan failed: rclone lsjson failed for remote directory Broken Vault: invalidRequest".into(),
                last_sync_at: 46,
                base_revision: "dir".into(),
                conflict_reason: String::new(),
            },
            PathState {
                path: "Docs".into(),
                is_dir: true,
                state: PathSyncState::PinnedLocal,
                size_bytes: 0,
                pinned: true,
                hydrated: false,
                dirty: false,
                error: String::new(),
                last_sync_at: 44,
                base_revision: "dir".into(),
                conflict_reason: String::new(),
            },
            PathState {
                path: "Docs/readme.md".into(),
                is_dir: false,
                state: PathSyncState::Conflict,
                size_bytes: 10,
                pinned: false,
                hydrated: true,
                dirty: true,
                error: String::new(),
                last_sync_at: 45,
                base_revision: "rev-1".into(),
                conflict_reason: "remote changed".into(),
            },
        ];

        store.replace_all(&snapshot).expect("replace");
        assert_eq!(store.all().expect("all"), snapshot);
        assert_eq!(
            store.get_many(&["Docs".into()]).expect("get"),
            vec![snapshot[1].clone()]
        );
    }
}
