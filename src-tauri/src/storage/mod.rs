//! Local history storage. The only module that talks to SQLite.
//!
//! Clipboard text is stored here and nowhere else. Errors from this module carry
//! no clipboard content, and callers must log only their `Display` text.

pub mod history;
pub mod paths;

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use history::{Entry, InsertOutcome, Source};

/// Schema version written to `PRAGMA user_version`.
const SCHEMA_VERSION: i64 = 1;

/// Managed Tauri state. A mutex serialises access to the single connection.
pub struct HistoryState(pub Mutex<Store>);

pub struct Store {
    conn: Connection,
}

/// Failure while opening or using the history database.
///
/// `Display` is generic on purpose. The wrapped error is kept for debugging
/// through `source()` only, and must not be printed next to clipboard data.
#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Db(rusqlite::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(_) => write!(f, "history folder or file could not be prepared"),
            StoreError::Db(_) => write!(f, "history database operation failed"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Io(e) => Some(e),
            StoreError::Db(e) => Some(e),
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Db(e)
    }
}

impl Store {
    /// Open or create the database file at `path`. The file is owner readable only.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        create_owner_only(path)?;
        let conn = Connection::open(path)?;
        Self::from_connection(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self, StoreError> {
        migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Remove every stored entry. The native clipboard is not modified.
    pub fn clear(&mut self) -> Result<usize, StoreError> {
        Ok(self.conn.execute("DELETE FROM entries", [])?)
    }

    pub fn insert(
        &mut self,
        text: &str,
        captured_at_ms: i64,
        source: &Source,
    ) -> Result<InsertOutcome, StoreError> {
        Ok(history::insert(
            &mut self.conn,
            text,
            captured_at_ms,
            source,
        )?)
    }

    pub fn get(&self, id: i64) -> Result<Option<Entry>, StoreError> {
        Ok(history::get(&self.conn, id)?)
    }

    pub fn list(&self, limit: i64) -> Result<Vec<Entry>, StoreError> {
        Ok(history::list(&self.conn, limit)?)
    }

    pub fn search(&self, query: &str, limit: i64) -> Result<Vec<Entry>, StoreError> {
        Ok(history::search(&self.conn, query, limit)?)
    }
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            captured_at_ms INTEGER NOT NULL,
            source_bundle_id TEXT,
            source_name TEXT
        );
        CREATE INDEX IF NOT EXISTS entries_captured_at ON entries (captured_at_ms);",
    )?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
}

/// Create the file with mode 0600 if it does not exist, then tighten an existing one.
#[cfg(unix)]
fn create_owner_only(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn create_owner_only(path: &Path) -> io::Result<()> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map(|_| ())
}
