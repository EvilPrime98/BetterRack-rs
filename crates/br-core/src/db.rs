//! A single SQLite connection behind a mutex. SQLite is single-writer, so one connection used from
//! `spawn_blocking` is enough. No WAL: existing databases do not use it.

use crate::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Open, creating the file and its parent folder if needed.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            conn: Mutex::new(Connection::open(path)?),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(Connection::open_in_memory()?),
        })
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// `ALTER TABLE .. ADD COLUMN`, ignoring "column already exists".
pub fn add_column_if_missing(conn: &Connection, table: &str, column_def: &str) {
    let _ = conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column_def}"), []);
}
