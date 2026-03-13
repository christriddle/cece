use crate::error::Result;
use rusqlite::Connection;
use std::path::Path;

pub mod agent;
pub mod config;
pub mod repo;
pub mod template;
pub mod workspace;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let db = Database { conn };
        db.run_migrations()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let db = Database { conn };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<()> {
        self.conn.execute_batch(include_str!("schema.sql"))?;
        // Add columns introduced after initial release (ignored if already present)
        let _ = self.conn.execute_batch(
            "ALTER TABLE workspaces ADD COLUMN cmux_workspace_id TEXT;",
        );
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}
