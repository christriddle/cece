/set# Cece CLI Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI tool that manages workspaces of Git worktrees and Claude Code AI agents, with optional Cmux integration for tab/workspace management.

**Architecture:** State stored in `~/.cece/cece.db` (SQLite). Workspaces are directories containing git worktrees created from repo paths the user provides. Agents are Claude Code sessions associated with a workspace. Cmux integration is optional and controls whether `ws switch` / `agent switch` uses the Cmux API.

**Tech Stack:** Rust, clap 4 (derive), rusqlite (bundled), thiserror, anyhow, dialoguer, comfy-table, serde + serde_json

---

## File Structure

```
Cargo.toml
src/
  main.rs               — entry point: parse CLI args, dispatch to handlers
  error.rs              — CeceError (thiserror), app-level Result alias
  db/
    mod.rs              — Database struct, open(), run_migrations()
    schema.sql          — embedded SQL schema (include_str!)
    workspace.rs        — workspace + workspace_repo CRUD
    agent.rs            — agent CRUD
    template.rs         — template CRUD
    config.rs           — key/value config get/set
    repo.rs             — known_repos CRUD
  cli/
    mod.rs              — top-level Parser + Subcommand enum
    init.rs             — handle_init()
    workspace.rs        — handle_ws()
    agent.rs            — handle_agent()
    template.rs         — handle_template()
    status.rs           — handle_status()
  cmux/
    mod.rs              — CmuxClient trait + implementations (cli, socket, noop)
  git/
    mod.rs              — worktree_add(), worktree_remove(), detect_default_branch()
tests/
  init_test.rs
  workspace_test.rs
  agent_test.rs
  template_test.rs
  status_test.rs
.github/
  workflows/
    ci.yml              — build + test on push
    release.yml         — build binaries on tag, upload to GitHub Release
```

---

## Chunk 1: Project Scaffolding

### Task 1: Cargo.toml and module stubs

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/error.rs`
- Create: `src/db/mod.rs`
- Create: `src/db/schema.sql`
- Create: `src/cli/mod.rs`
- Create: `src/cmux/mod.rs`
- Create: `src/git/mod.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "cece"
version = "0.1.0"
edition = "2021"
description = "Manage workspaces of Git repositories and AI agents"
license = "MIT"
repository = "https://github.com/christriddle/cece"

[[bin]]
name = "cece"
path = "src/main.rs"

[dependencies]
anyhow = "1"
thiserror = "2"
clap = { version = "4", features = ["derive", "env"] }
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dialoguer = "0.11"
comfy-table = "7"
dirs = "5"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

- [ ] **Step 2: Create src/error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CeceError {
    #[error("workspace '{0}' not found")]
    WorkspaceNotFound(String),
    #[error("agent '{0}' not found")]
    AgentNotFound(String),
    #[error("template '{0}' not found")]
    TemplateNotFound(String),
    #[error("workspace '{0}' already exists")]
    WorkspaceExists(String),
    #[error("agent '{0}' already exists")]
    AgentExists(String),
    #[error("cece is not initialized — run `cece init` first")]
    NotInitialized,
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("git error: {0}")]
    Git(String),
    #[error("cmux error: {0}")]
    Cmux(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CeceError>;
```

- [ ] **Step 3: Create src/db/schema.sql**

```sql
CREATE TABLE IF NOT EXISTS config (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS known_repos (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS workspaces (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT UNIQUE NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS workspace_repos (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id  INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    repo_path     TEXT NOT NULL,
    branch        TEXT NOT NULL,
    worktree_path TEXT NOT NULL,
    UNIQUE(workspace_id, repo_path)
);

CREATE TABLE IF NOT EXISTS agents (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT NOT NULL,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    working_dir  TEXT NOT NULL,
    session_id   TEXT,
    last_request TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(name, workspace_id)
);

CREATE TABLE IF NOT EXISTS templates (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT UNIQUE NOT NULL,
    branch_template TEXT NOT NULL,
    repo_paths      TEXT NOT NULL  -- JSON array of repo paths
);
```

- [ ] **Step 4: Create src/db/mod.rs**

```rust
use crate::error::{CeceError, Result};
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
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}
```

- [ ] **Step 5: Create src/cli/mod.rs stub**

```rust
use clap::{Parser, Subcommand};

pub mod agent;
pub mod init;
pub mod status;
pub mod template;
pub mod workspace;

#[derive(Parser)]
#[command(name = "cece", about = "Manage workspaces of Git repos and AI agents", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize cece in your home directory
    Init,
    /// Manage workspaces
    #[command(subcommand, name = "ws")]
    Workspace(workspace::WorkspaceCommands),
    /// Manage agents in the current workspace
    #[command(subcommand)]
    Agent(agent::AgentCommands),
    /// Manage workspace templates
    #[command(subcommand)]
    Template(template::TemplateCommands),
    /// Show status of all workspaces and agents
    Status,
}
```

- [ ] **Step 6: Create src/main.rs**

```rust
mod cli;
mod cmux;
mod db;
mod error;
mod git;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => cli::init::handle_init()?,
        Commands::Workspace(cmd) => cli::workspace::handle_ws(cmd)?,
        Commands::Agent(cmd) => cli::agent::handle_agent(cmd)?,
        Commands::Template(cmd) => cli::template::handle_template(cmd)?,
        Commands::Status => cli::status::handle_status()?,
    }
    Ok(())
}
```

- [ ] **Step 7: Create stub files for remaining modules**

Create `src/cmux/mod.rs`:
```rust
pub struct NoopCmux;
```

Create `src/git/mod.rs`:
```rust
// git worktree helpers — implemented in Task 3
```

Create stub handler files that match the signatures used in `main.rs`:

`src/cli/init.rs`:
```rust
pub fn handle_init() -> anyhow::Result<()> { Ok(()) }
```

`src/cli/workspace.rs`:
```rust
use clap::Subcommand;
#[derive(Subcommand)]
pub enum WorkspaceCommands {}
pub fn handle_ws(_cmd: WorkspaceCommands) -> anyhow::Result<()> { Ok(()) }
```

`src/cli/agent.rs`:
```rust
use clap::Subcommand;
#[derive(Subcommand)]
pub enum AgentCommands {}
pub fn handle_agent(_cmd: AgentCommands) -> anyhow::Result<()> { Ok(()) }
```

`src/cli/template.rs`:
```rust
use clap::Subcommand;
#[derive(Subcommand)]
pub enum TemplateCommands {}
pub fn handle_template(_cmd: TemplateCommands) -> anyhow::Result<()> { Ok(()) }
```

`src/cli/status.rs`:
```rust
pub fn handle_status() -> anyhow::Result<()> { Ok(()) }
```

Also add cmux free-function stubs so later chunks compile before Task 7:

`src/cmux/mod.rs`:
```rust
use anyhow::Result;
use std::path::Path;

pub fn select_workspace(_name: &str) -> Result<()> { Ok(()) }
pub fn new_agent_tab(_workspace: &str, _agent: &str, _dir: &Path) -> Result<String> {
    Ok(String::new())
}
pub fn select_agent_tab(_workspace: &str, _agent: &str) -> Result<()> { Ok(()) }
```

- [ ] **Step 8: Verify it compiles**

```bash
cargo build
```
Expected: compiles with possible unused import warnings, no errors.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "chore: initial project scaffolding"
```

---

### Task 2: DB CRUD modules

**Files:**
- Create: `src/db/config.rs`
- Create: `src/db/repo.rs`
- Create: `src/db/workspace.rs`
- Create: `src/db/agent.rs`
- Create: `src/db/template.rs`

- [ ] **Step 1: Write failing tests for config get/set**

Create `src/db/config.rs`:
```rust
use crate::db::Database;
use crate::error::Result;

pub fn set(db: &Database, key: &str, value: &str) -> Result<()> {
    db.conn().execute(
        "INSERT INTO config (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    )?;
    Ok(())
}

pub fn get(db: &Database, key: &str) -> Result<Option<String>> {
    let mut stmt = db.conn().prepare("SELECT value FROM config WHERE key = ?1")?;
    let mut rows = stmt.query([key])?;
    Ok(rows.next()?.map(|r| r.get(0)).transpose()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let db = Database::open_in_memory().unwrap();
        set(&db, "branch_template", "{initials}-{ticket}-{desc}").unwrap();
        assert_eq!(
            get(&db, "branch_template").unwrap(),
            Some("{initials}-{ticket}-{desc}".to_string())
        );
    }

    #[test]
    fn test_get_missing_key_returns_none() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(get(&db, "nonexistent").unwrap(), None);
    }

    #[test]
    fn test_set_overwrites() {
        let db = Database::open_in_memory().unwrap();
        set(&db, "key", "v1").unwrap();
        set(&db, "key", "v2").unwrap();
        assert_eq!(get(&db, "key").unwrap(), Some("v2".to_string()));
    }
}
```

- [ ] **Step 2: Run config tests**

```bash
cargo test db::config
```
Expected: 3 tests pass.

- [ ] **Step 3: Write and test known_repos CRUD**

Create `src/db/repo.rs`:
```rust
use crate::db::Database;
use crate::error::Result;

pub fn add(db: &Database, path: &str) -> Result<()> {
    db.conn().execute(
        "INSERT OR IGNORE INTO known_repos (path) VALUES (?1)",
        [path],
    )?;
    Ok(())
}

pub fn list(db: &Database) -> Result<Vec<String>> {
    let mut stmt = db.conn().prepare("SELECT path FROM known_repos ORDER BY path")?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

pub fn remove(db: &Database, path: &str) -> Result<()> {
    db.conn().execute("DELETE FROM known_repos WHERE path = ?1", [path])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_list() {
        let db = Database::open_in_memory().unwrap();
        add(&db, "/home/user/repos/frontend").unwrap();
        add(&db, "/home/user/repos/backend").unwrap();
        let repos = list(&db).unwrap();
        assert_eq!(repos.len(), 2);
        assert!(repos.contains(&"/home/user/repos/frontend".to_string()));
    }

    #[test]
    fn test_add_duplicate_is_ignored() {
        let db = Database::open_in_memory().unwrap();
        add(&db, "/home/user/repos/frontend").unwrap();
        add(&db, "/home/user/repos/frontend").unwrap();
        assert_eq!(list(&db).unwrap().len(), 1);
    }

    #[test]
    fn test_remove() {
        let db = Database::open_in_memory().unwrap();
        add(&db, "/home/user/repos/frontend").unwrap();
        remove(&db, "/home/user/repos/frontend").unwrap();
        assert!(list(&db).unwrap().is_empty());
    }
}
```

- [ ] **Step 4: Run repo tests**

```bash
cargo test db::repo
```
Expected: 3 tests pass.

- [ ] **Step 5: Write and test workspace CRUD**

Create `src/db/workspace.rs`:
```rust
use crate::db::Database;
use crate::error::{CeceError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceRepo {
    pub id: i64,
    pub workspace_id: i64,
    pub repo_path: String,
    pub branch: String,
    pub worktree_path: String,
}

pub fn create(db: &Database, name: &str) -> Result<i64> {
    db.conn().execute("INSERT INTO workspaces (name) VALUES (?1)", [name])?;
    Ok(db.conn().last_insert_rowid())
}

pub fn get_by_name(db: &Database, name: &str) -> Result<Workspace> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, created_at FROM workspaces WHERE name = ?1",
    )?;
    stmt.query_row([name], |r| {
        Ok(Workspace { id: r.get(0)?, name: r.get(1)?, created_at: r.get(2)? })
    })
    .map_err(|_| CeceError::WorkspaceNotFound(name.to_string()))
}

pub fn list(db: &Database) -> Result<Vec<Workspace>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, created_at FROM workspaces ORDER BY name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Workspace { id: r.get(0)?, name: r.get(1)?, created_at: r.get(2)? })
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

pub fn delete(db: &Database, name: &str) -> Result<()> {
    let rows = db.conn().execute("DELETE FROM workspaces WHERE name = ?1", [name])?;
    if rows == 0 {
        return Err(CeceError::WorkspaceNotFound(name.to_string()));
    }
    Ok(())
}

pub fn add_repo(
    db: &Database,
    workspace_id: i64,
    repo_path: &str,
    branch: &str,
    worktree_path: &str,
) -> Result<()> {
    db.conn().execute(
        "INSERT INTO workspace_repos (workspace_id, repo_path, branch, worktree_path)
         VALUES (?1, ?2, ?3, ?4)",
        (workspace_id, repo_path, branch, worktree_path),
    )?;
    Ok(())
}

pub fn get_repos(db: &Database, workspace_id: i64) -> Result<Vec<WorkspaceRepo>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, workspace_id, repo_path, branch, worktree_path
         FROM workspace_repos WHERE workspace_id = ?1",
    )?;
    let rows = stmt.query_map([workspace_id], |r| {
        Ok(WorkspaceRepo {
            id: r.get(0)?,
            workspace_id: r.get(1)?,
            repo_path: r.get(2)?,
            branch: r.get(3)?,
            worktree_path: r.get(4)?,
        })
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get() {
        let db = Database::open_in_memory().unwrap();
        create(&db, "my-workspace").unwrap();
        let ws = get_by_name(&db, "my-workspace").unwrap();
        assert_eq!(ws.name, "my-workspace");
    }

    #[test]
    fn test_list() {
        let db = Database::open_in_memory().unwrap();
        create(&db, "alpha").unwrap();
        create(&db, "beta").unwrap();
        let workspaces = list(&db).unwrap();
        assert_eq!(workspaces.len(), 2);
    }

    #[test]
    fn test_delete() {
        let db = Database::open_in_memory().unwrap();
        create(&db, "to-delete").unwrap();
        delete(&db, "to-delete").unwrap();
        assert!(list(&db).unwrap().is_empty());
    }

    #[test]
    fn test_delete_nonexistent_errors() {
        let db = Database::open_in_memory().unwrap();
        let result = delete(&db, "ghost");
        assert!(matches!(result, Err(CeceError::WorkspaceNotFound(_))));
    }

    #[test]
    fn test_add_and_get_repos() {
        let db = Database::open_in_memory().unwrap();
        let ws_id = create(&db, "ws").unwrap();
        add_repo(&db, ws_id, "/repos/frontend", "main", "/cece/ws/frontend").unwrap();
        let repos = get_repos(&db, ws_id).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].branch, "main");
    }
}
```

- [ ] **Step 6: Run workspace tests**

```bash
cargo test db::workspace
```
Expected: 5 tests pass.

- [ ] **Step 7: Write and test agent CRUD**

Create `src/db/agent.rs`:
```rust
use crate::db::Database;
use crate::error::{CeceError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    pub id: i64,
    pub name: String,
    pub workspace_id: i64,
    pub working_dir: String,
    pub session_id: Option<String>,
    pub last_request: Option<String>,
    pub created_at: String,
}

pub fn create(db: &Database, name: &str, workspace_id: i64, working_dir: &str) -> Result<i64> {
    db.conn().execute(
        "INSERT INTO agents (name, workspace_id, working_dir) VALUES (?1, ?2, ?3)",
        (name, workspace_id, working_dir),
    )?;
    Ok(db.conn().last_insert_rowid())
}

pub fn get_by_name(db: &Database, name: &str, workspace_id: i64) -> Result<Agent> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, workspace_id, working_dir, session_id, last_request, created_at
         FROM agents WHERE name = ?1 AND workspace_id = ?2",
    )?;
    stmt.query_row((name, workspace_id), |r| {
        Ok(Agent {
            id: r.get(0)?,
            name: r.get(1)?,
            workspace_id: r.get(2)?,
            working_dir: r.get(3)?,
            session_id: r.get(4)?,
            last_request: r.get(5)?,
            created_at: r.get(6)?,
        })
    })
    .map_err(|_| CeceError::AgentNotFound(name.to_string()))
}

pub fn list(db: &Database, workspace_id: i64) -> Result<Vec<Agent>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, workspace_id, working_dir, session_id, last_request, created_at
         FROM agents WHERE workspace_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map([workspace_id], |r| {
        Ok(Agent {
            id: r.get(0)?,
            name: r.get(1)?,
            workspace_id: r.get(2)?,
            working_dir: r.get(3)?,
            session_id: r.get(4)?,
            last_request: r.get(5)?,
            created_at: r.get(6)?,
        })
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

pub fn delete(db: &Database, name: &str, workspace_id: i64) -> Result<()> {
    let rows = db.conn().execute(
        "DELETE FROM agents WHERE name = ?1 AND workspace_id = ?2",
        (name, workspace_id),
    )?;
    if rows == 0 {
        return Err(CeceError::AgentNotFound(name.to_string()));
    }
    Ok(())
}

pub fn update_session(db: &Database, id: i64, session_id: &str, last_request: Option<&str>) -> Result<()> {
    db.conn().execute(
        "UPDATE agents SET session_id = ?1, last_request = ?2 WHERE id = ?3",
        (session_id, last_request, id),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::workspace;

    fn setup() -> (Database, i64) {
        let db = Database::open_in_memory().unwrap();
        let ws_id = workspace::create(&db, "ws").unwrap();
        (db, ws_id)
    }

    #[test]
    fn test_create_and_get() {
        let (db, ws_id) = setup();
        create(&db, "agent1", ws_id, "/cece/ws/frontend").unwrap();
        let agent = get_by_name(&db, "agent1", ws_id).unwrap();
        assert_eq!(agent.name, "agent1");
        assert_eq!(agent.session_id, None);
    }

    #[test]
    fn test_list() {
        let (db, ws_id) = setup();
        create(&db, "a1", ws_id, "/cece/ws").unwrap();
        create(&db, "a2", ws_id, "/cece/ws").unwrap();
        assert_eq!(list(&db, ws_id).unwrap().len(), 2);
    }

    #[test]
    fn test_delete() {
        let (db, ws_id) = setup();
        create(&db, "a1", ws_id, "/cece/ws").unwrap();
        delete(&db, "a1", ws_id).unwrap();
        assert!(list(&db, ws_id).unwrap().is_empty());
    }

    #[test]
    fn test_update_session() {
        let (db, ws_id) = setup();
        let id = create(&db, "a1", ws_id, "/cece/ws").unwrap();
        update_session(&db, id, "ses-123", Some("Fix the auth bug")).unwrap();
        let agent = get_by_name(&db, "a1", ws_id).unwrap();
        assert_eq!(agent.session_id, Some("ses-123".to_string()));
        assert_eq!(agent.last_request, Some("Fix the auth bug".to_string()));
    }
}
```

- [ ] **Step 8: Write and test template CRUD**

Create `src/db/template.rs`:
```rust
use crate::db::Database;
use crate::error::{CeceError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub id: i64,
    pub name: String,
    pub branch_template: String,
    pub repo_paths: Vec<String>,  // deserialized from JSON
}

pub fn create(db: &Database, name: &str, branch_template: &str, repo_paths: &[String]) -> Result<i64> {
    let repo_paths_json = serde_json::to_string(repo_paths).expect("serialization is infallible");
    db.conn().execute(
        "INSERT INTO templates (name, branch_template, repo_paths) VALUES (?1, ?2, ?3)",
        (name, branch_template, &repo_paths_json),
    )?;
    Ok(db.conn().last_insert_rowid())
}

pub fn get_by_name(db: &Database, name: &str) -> Result<Template> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, branch_template, repo_paths FROM templates WHERE name = ?1",
    )?;
    stmt.query_row([name], |r| {
        let repo_paths_json: String = r.get(3)?;
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, repo_paths_json))
    })
    .map_err(|_| CeceError::TemplateNotFound(name.to_string()))
    .and_then(|(id, name, branch_template, json): (i64, String, String, String)| {
        let repo_paths = serde_json::from_str(&json)
            .map_err(|e| CeceError::Git(format!("invalid template data: {e}")))?;
        Ok(Template { id, name, branch_template, repo_paths })
    })
}

pub fn list(db: &Database) -> Result<Vec<Template>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, branch_template, repo_paths FROM templates ORDER BY name",
    )?;
    let rows = stmt.query_map([], |r| {
        let json: String = r.get(3)?;
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, json))
    })?;
    rows.map(|r| {
        let (id, name, branch_template, json): (i64, String, String, String) = r?;
        let repo_paths = serde_json::from_str(&json)
            .map_err(|e| CeceError::Git(format!("invalid template data: {e}")))?;
        Ok(Template { id, name, branch_template, repo_paths })
    })
    .collect()
}

pub fn delete(db: &Database, name: &str) -> Result<()> {
    let rows = db.conn().execute("DELETE FROM templates WHERE name = ?1", [name])?;
    if rows == 0 {
        return Err(CeceError::TemplateNotFound(name.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get() {
        let db = Database::open_in_memory().unwrap();
        let repos = vec!["/repos/frontend".to_string()];
        create(&db, "feature", "{initials}-{ticket}-{desc}", &repos).unwrap();
        let t = get_by_name(&db, "feature").unwrap();
        assert_eq!(t.branch_template, "{initials}-{ticket}-{desc}");
        assert_eq!(t.repo_paths, repos);
    }

    #[test]
    fn test_delete() {
        let db = Database::open_in_memory().unwrap();
        create(&db, "t1", "main", &[]).unwrap();
        delete(&db, "t1").unwrap();
        assert!(list(&db).unwrap().is_empty());
    }
}
```

- [ ] **Step 9: Run all DB tests**

```bash
cargo test db::
```
Expected: all tests pass.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat: add database layer with schema and CRUD modules"
```

---

## Chunk 2: Init Command & Config

### Task 3: cece init

**Files:**
- Modify: `src/cli/init.rs`
- Modify: `src/main.rs` (add db_path helper)

- [ ] **Step 1: Add cece_dir() and db_path() helpers to main.rs**

Add to `src/main.rs`:
```rust
use std::path::PathBuf;

pub fn cece_dir() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    Ok(home.join(".cece"))
}

pub fn db_path() -> anyhow::Result<PathBuf> {
    Ok(cece_dir()?.join("cece.db"))
}

pub fn open_db() -> anyhow::Result<db::Database> {
    let path = db_path()?;
    if !path.exists() {
        anyhow::bail!("cece is not initialized — run `cece init` first");
    }
    Ok(db::Database::open(path)?)
}
```

- [ ] **Step 2: Write failing test for init**

Create `tests/init_test.rs`:
```rust
use assert_cmd::Command;
use tempfile::TempDir;

fn cece_cmd(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("cece").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn test_init_creates_db() {
    let home = TempDir::new().unwrap();
    cece_cmd(home.path())
        .arg("init")
        .env("CECE_NON_INTERACTIVE", "1")
        .assert()
        .success();
    assert!(home.path().join(".cece").join("cece.db").exists());
}

#[test]
fn test_init_twice_is_idempotent() {
    let home = TempDir::new().unwrap();
    for _ in 0..2 {
        cece_cmd(home.path())
            .arg("init")
            .env("CECE_NON_INTERACTIVE", "1")
            .assert()
            .success();
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

```bash
cargo test --test init_test
```
Expected: FAIL (init does nothing yet).

- [ ] **Step 4: Implement handle_init()**

Replace `src/cli/init.rs`:
```rust
use crate::{cece_dir, db, db::config};
use anyhow::{Context, Result};
use dialoguer::{Confirm, Input};
use std::fs;

pub fn handle_init() -> Result<()> {
    let dir = cece_dir()?;
    let non_interactive = std::env::var("CECE_NON_INTERACTIVE").is_ok();

    if dir.exists() {
        if !non_interactive {
            println!("cece is already initialized at {}", dir.display());
        }
    } else {
        fs::create_dir_all(&dir).context("failed to create ~/.cece")?;
        println!("Initialized cece at {}", dir.display());
    }

    let db_path = dir.join("cece.db");
    let db = db::Database::open(&db_path)?;

    if non_interactive {
        return Ok(());
    }

    // Branch template
    let existing_template = config::get(&db, "branch_template")?;
    let default_template = existing_template.as_deref().unwrap_or("{initials}-{ticket}-{desc}");
    let branch_template: String = Input::new()
        .with_prompt("Branch name template")
        .with_initial_text(default_template)
        .interact_text()?;
    config::set(&db, "branch_template", &branch_template)?;

    // Cmux
    let use_cmux: bool = Confirm::new()
        .with_prompt("Enable Cmux integration?")
        .default(config::get(&db, "cmux_enabled")?.as_deref() == Some("true"))
        .interact()?;
    config::set(&db, "cmux_enabled", if use_cmux { "true" } else { "false" })?;

    println!("Configuration saved.");
    Ok(())
}
```

- [ ] **Step 5: Run init tests**

```bash
cargo test --test init_test
```
Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: implement cece init command"
```

---

## Chunk 3: Git Worktree Helpers

### Task 4: git module

**Files:**
- Modify: `src/git/mod.rs`

- [ ] **Step 1: Write failing tests**

Add to `src/git/mod.rs`:
```rust
use crate::error::{CeceError, Result};
use std::path::Path;
use std::process::Command;

pub fn detect_default_branch(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["-C", &repo_path.to_string_lossy(), "symbolic-ref", "refs/remotes/origin/HEAD"])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_start_matches("refs/remotes/origin/")
            .to_string();
        return Ok(branch);
    }

    // Fallback: check for main then master
    for branch in ["main", "master"] {
        let check = Command::new("git")
            .args(["-C", &repo_path.to_string_lossy(), "rev-parse", "--verify", branch])
            .output()
            .map_err(|e| CeceError::Git(e.to_string()))?;
        if check.status.success() {
            return Ok(branch.to_string());
        }
    }

    Err(CeceError::Git("cannot determine default branch".to_string()))
}

pub fn worktree_add(repo_path: &Path, worktree_path: &Path, branch: &str) -> Result<()> {
    std::fs::create_dir_all(worktree_path.parent().unwrap_or(worktree_path))
        .map_err(CeceError::Io)?;
    let output = Command::new("git")
        .args([
            "-C",
            &repo_path.to_string_lossy(),
            "worktree",
            "add",
            "-b",
            branch,
            &worktree_path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CeceError::Git(format!("git worktree add failed: {stderr}")));
    }
    Ok(())
}

pub fn worktree_remove(repo_path: &Path, worktree_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .args([
            "-C",
            &repo_path.to_string_lossy(),
            "worktree",
            "remove",
            "--force",
            &worktree_path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CeceError::Git(format!("git worktree remove failed: {stderr}")));
    }
    Ok(())
}

pub fn expand_branch_template(template: &str, vars: &std::collections::HashMap<&str, &str>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_expand_branch_template() {
        let mut vars = HashMap::new();
        vars.insert("initials", "cr");
        vars.insert("ticket", "OPEN-123");
        vars.insert("desc", "fix-auth");
        let result = expand_branch_template("{initials}-{ticket}-{desc}", &vars);
        assert_eq!(result, "cr-OPEN-123-fix-auth");
    }

    #[test]
    fn test_expand_partial_template() {
        let mut vars = HashMap::new();
        vars.insert("initials", "cr");
        let result = expand_branch_template("{initials}-feature", &vars);
        assert_eq!(result, "cr-feature");
    }
}
```

- [ ] **Step 2: Run git module tests**

```bash
cargo test git::
```
Expected: 2 tests pass (template tests; worktree tests require a real git repo so skip for now).

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: add git worktree helpers"
```

---

## Chunk 4: Workspace Commands

### Task 5: ws create, list, delete, switch

**Files:**
- Modify: `src/cli/workspace.rs`

- [ ] **Step 1: Define workspace CLI structs**

Replace `src/cli/workspace.rs`:
```rust
use anyhow::Result;
use clap::{Args, Subcommand};
use comfy_table::{Cell, Table};
use dialoguer::{Input, MultiSelect};
use std::collections::HashMap;

use crate::{cece_dir, db, db::config, db::repo, db::workspace, git, open_db};

#[derive(Subcommand)]
pub enum WorkspaceCommands {
    /// Create a new workspace
    Create {
        name: String,
        /// Repos to include (paths on disk). If omitted, prompted interactively.
        #[arg(long, num_args = 1..)]
        repos: Vec<String>,
        /// Branch name override (skips template expansion)
        #[arg(long)]
        branch: Option<String>,
    },
    /// List all workspaces
    List,
    /// Delete a workspace and its worktrees
    Delete { name: String },
    /// Switch to a workspace (prints path, or uses Cmux if configured)
    Switch { name: String },
}

pub fn handle_ws(cmd: WorkspaceCommands) -> Result<()> {
    match cmd {
        WorkspaceCommands::Create { name, repos, branch } => create(&name, repos, branch),
        WorkspaceCommands::List => list(),
        WorkspaceCommands::Delete { name } => delete(&name),
        WorkspaceCommands::Switch { name } => switch(&name),
    }
}

fn create(name: &str, mut repo_paths: Vec<String>, branch_override: Option<String>) -> Result<()> {
    let db = open_db()?;

    // Gather repos interactively if not provided
    if repo_paths.is_empty() {
        let known = repo::list(&db)?;
        if known.is_empty() {
            let path: String = Input::new()
                .with_prompt("Enter a repo path")
                .interact_text()?;
            repo_paths.push(path);
        } else {
            let selections = MultiSelect::new()
                .with_prompt("Select repos to include (space to toggle, enter to confirm)")
                .items(&known)
                .interact()?;
            for i in selections {
                repo_paths.push(known[i].clone());
            }
            let add_more: String = Input::new()
                .with_prompt("Add another repo path (blank to skip)")
                .allow_empty(true)
                .interact_text()?;
            if !add_more.is_empty() {
                repo_paths.push(add_more);
            }
        }
    }

    if repo_paths.is_empty() {
        anyhow::bail!("no repos selected");
    }

    // Determine branch name
    let branch = match branch_override {
        Some(b) => b,
        None => {
            let template = config::get(&db, "branch_template")?
                .unwrap_or_else(|| "{initials}-{ticket}-{desc}".to_string());

            if template.contains('{') {
                let initials: String = Input::new().with_prompt("Your initials").interact_text()?;
                let ticket: String = Input::new().with_prompt("Ticket number").allow_empty(true).interact_text()?;
                let desc: String = Input::new().with_prompt("Short description").interact_text()?;
                let mut vars = HashMap::new();
                vars.insert("initials", initials.as_str());
                vars.insert("ticket", ticket.as_str());
                vars.insert("desc", desc.as_str());
                git::expand_branch_template(&template, &vars)
            } else {
                template
            }
        }
    };

    let ws_id = workspace::create(&db, name)?;
    let ws_dir = cece_dir()?.join("workspaces").join(name);

    for repo_path_str in &repo_paths {
        let repo_path = std::path::Path::new(repo_path_str);
        let repo_name = repo_path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());
        let worktree_path = ws_dir.join(&repo_name);

        git::worktree_add(repo_path, &worktree_path, &branch)?;
        workspace::add_repo(&db, ws_id, repo_path_str, &branch, &worktree_path.to_string_lossy())?;
        repo::add(&db, repo_path_str)?;  // remember it

        println!("  added {} → {}", repo_name, worktree_path.display());
    }

    println!("Workspace '{}' created (branch: {})", name, branch);
    Ok(())
}

fn list() -> Result<()> {
    let db = open_db()?;
    let workspaces = workspace::list(&db)?;

    if workspaces.is_empty() {
        println!("No workspaces. Run `cece ws create <name>` to create one.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(["Name", "Repos", "Created"]);
    for ws in &workspaces {
        let repos = workspace::get_repos(&db, ws.id)?;
        let repo_names: Vec<_> = repos.iter()
            .map(|r| std::path::Path::new(&r.repo_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default())
            .collect();
        table.add_row([
            Cell::new(&ws.name),
            Cell::new(repo_names.join(", ")),
            Cell::new(&ws.created_at[..10]),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn delete(name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, name)?;
    let repos = workspace::get_repos(&db, ws.id)?;

    for r in &repos {
        let repo_path = std::path::Path::new(&r.repo_path);
        let worktree_path = std::path::Path::new(&r.worktree_path);
        if worktree_path.exists() {
            git::worktree_remove(repo_path, worktree_path)
                .unwrap_or_else(|e| eprintln!("warning: {e}"));
        }
    }

    // Remove workspace directory
    let ws_dir = cece_dir()?.join("workspaces").join(name);
    if ws_dir.exists() {
        std::fs::remove_dir_all(&ws_dir).ok();
    }

    workspace::delete(&db, name)?;
    println!("Workspace '{}' deleted.", name);
    Ok(())
}

fn switch(name: &str) -> Result<()> {
    let db = open_db()?;
    workspace::get_by_name(&db, name)?;  // validates it exists

    let ws_dir = cece_dir()?.join("workspaces").join(name);
    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");

    if cmux_enabled {
        crate::cmux::select_workspace(name)?;
        println!("Switched to workspace '{}' in Cmux.", name);
    } else {
        println!("{}", ws_dir.display());
        eprintln!("(Tip: use `cd $(cece ws switch {})` to change directories)", name);
    }
    Ok(())
}
```

- [ ] **Step 2: Write integration test**

Create `tests/workspace_test.rs`:
```rust
use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn init_cece(home: &std::path::Path) {
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home)
        .env("CECE_NON_INTERACTIVE", "1")
        .arg("init")
        .assert()
        .success();
}

#[test]
fn test_ws_list_empty() {
    let home = TempDir::new().unwrap();
    init_cece(home.path());
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .args(["ws", "list"])
        .assert()
        .success()
        .stdout(contains("No workspaces"));
}
```

- [ ] **Step 3: Run workspace integration tests**

```bash
cargo test --test workspace_test
```
Expected: passes.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: implement workspace commands (create, list, delete, switch)"
```

---

## Chunk 5: Agent Commands

### Task 6: agent create, list, delete, switch, logs, watch

**Files:**
- Create: `src/claude/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/cli/agent.rs`

- [ ] **Step 1: Create src/claude/mod.rs and register it in main.rs**

The `claude` module must exist before `agent.rs` can reference `crate::claude::*`.

Create `src/claude/mod.rs`:
```rust
use anyhow::Result;
use std::path::Path;
use std::time::Duration;
use std::thread;

/// Encode a directory path into the format Claude Code uses for session directories.
/// Claude Code stores sessions under ~/.claude/projects/<encoded-path>/
fn encode_project_path(dir: &Path) -> String {
    dir.to_string_lossy().replace('/', "-").trim_start_matches('-').to_string()
}

fn claude_session_dir(working_dir: &str) -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let encoded = encode_project_path(Path::new(working_dir));
    Some(home.join(".claude").join("projects").join(encoded))
}

/// Read recent session log entries from Claude Code's session storage.
pub fn read_session_logs(working_dir: &str) -> Result<Vec<String>> {
    let Some(session_dir) = claude_session_dir(working_dir) else {
        return Ok(vec![]);
    };
    if !session_dir.exists() {
        return Ok(vec![]);
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&session_dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("jsonl") {
            let content = std::fs::read_to_string(entry.path())?;
            for line in content.lines().rev().take(20) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
                        entries.push(msg.to_string());
                    }
                }
            }
        }
    }
    Ok(entries)
}

/// Poll until no Claude Code session file has been modified in the last 5 seconds.
/// This is a best-effort heuristic.
pub fn wait_until_idle(working_dir: &str) -> Result<()> {
    let Some(session_dir) = claude_session_dir(working_dir) else {
        anyhow::bail!("no Claude Code session directory found for {working_dir}");
    };

    loop {
        thread::sleep(Duration::from_secs(2));
        let most_recent = std::fs::read_dir(&session_dir)?
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
            .max();

        if let Some(modified) = most_recent {
            let age = modified.elapsed().unwrap_or(Duration::MAX);
            if age > Duration::from_secs(5) {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }
}
```

Add `pub mod claude;` to `src/main.rs` alongside the other `mod` declarations.

- [ ] **Step 2: Define agent CLI structs and stubs**

Replace `src/cli/agent.rs`:
```rust
use anyhow::Result;
use clap::Subcommand;
use comfy_table::{Cell, Table};
use std::path::PathBuf;

use crate::{db::agent, db::config, db::workspace, open_db};

#[derive(Subcommand)]
pub enum AgentCommands {
    /// Create a new agent in the current workspace
    Create {
        name: String,
        /// Workspace to create the agent in
        #[arg(long)]
        workspace: String,
        /// Working directory (defaults to workspace dir)
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// List agents in a workspace
    List {
        /// Workspace name
        #[arg(long)]
        workspace: String,
    },
    /// Delete an agent
    Delete {
        name: String,
        #[arg(long)]
        workspace: String,
    },
    /// Switch to (open/focus) an agent
    Switch {
        name: String,
        #[arg(long)]
        workspace: String,
    },
    /// Show session history for an agent
    Logs {
        name: String,
        #[arg(long)]
        workspace: String,
    },
    /// Wait until an agent is idle
    Watch {
        name: String,
        #[arg(long)]
        workspace: String,
    },
}

pub fn handle_agent(cmd: AgentCommands) -> Result<()> {
    match cmd {
        AgentCommands::Create { name, workspace, dir } => create(&name, &workspace, dir),
        AgentCommands::List { workspace } => list(&workspace),
        AgentCommands::Delete { name, workspace } => delete(&name, &workspace),
        AgentCommands::Switch { name, workspace } => switch(&name, &workspace),
        AgentCommands::Logs { name, workspace } => logs(&name, &workspace),
        AgentCommands::Watch { name, workspace } => watch(&name, &workspace),
    }
}

fn create(name: &str, workspace_name: &str, dir_override: Option<PathBuf>) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    let ws_dir = crate::cece_dir()?.join("workspaces").join(workspace_name);
    let working_dir = dir_override.unwrap_or(ws_dir);

    let id = agent::create(&db, name, ws.id, &working_dir.to_string_lossy())?;
    println!("Agent '{}' created in workspace '{}'.", name, workspace_name);

    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");
    if cmux_enabled {
        let session_id = crate::cmux::new_agent_tab(workspace_name, name, &working_dir)?;
        agent::update_session(&db, id, &session_id, None)?;
        println!("Opened in Cmux tab.");
    } else {
        println!("Launch Claude Code manually:");
        println!("  cd {} && claude", working_dir.display());
    }
    Ok(())
}

fn list(workspace_name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    let agents = agent::list(&db, ws.id)?;

    if agents.is_empty() {
        println!("No agents in workspace '{workspace_name}'.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(["Name", "Working Dir", "Session ID", "Last Request"]);
    for a in &agents {
        table.add_row([
            Cell::new(&a.name),
            Cell::new(&a.working_dir),
            Cell::new(a.session_id.as_deref().unwrap_or("—")),
            Cell::new(a.last_request.as_deref().unwrap_or("—")),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn delete(name: &str, workspace_name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    agent::delete(&db, name, ws.id)?;
    println!("Agent '{}' deleted.", name);
    Ok(())
}

fn switch(name: &str, workspace_name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    let a = agent::get_by_name(&db, name, ws.id)?;

    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");
    if cmux_enabled {
        crate::cmux::select_agent_tab(workspace_name, name)?;
        println!("Switched to agent '{}' in Cmux.", name);
    } else {
        println!("{}", a.working_dir);
        eprintln!("(Tip: cd to that directory and run `claude --continue`)");
    }
    Ok(())
}

fn logs(name: &str, workspace_name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    let a = agent::get_by_name(&db, name, ws.id)?;

    let logs = crate::claude::read_session_logs(&a.working_dir)?;
    if logs.is_empty() {
        println!("No session logs found for agent '{name}'.");
    } else {
        for entry in &logs {
            println!("{entry}");
        }
    }
    Ok(())
}

fn watch(name: &str, workspace_name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    let a = agent::get_by_name(&db, name, ws.id)?;

    println!("Watching agent '{name}'... (Ctrl+C to stop)");
    crate::claude::wait_until_idle(&a.working_dir)?;
    println!("Agent '{name}' is idle.");
    Ok(())
}
```

- [ ] **Step 3: Write agent integration test**

Create `tests/agent_test.rs`:
```rust
use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn init_cece(home: &std::path::Path) {
    Command::cargo_bin("cece").unwrap()
        .env("HOME", home).env("CECE_NON_INTERACTIVE", "1")
        .arg("init").assert().success();
}

#[test]
fn test_agent_list_no_workspace_errors() {
    let home = TempDir::new().unwrap();
    init_cece(home.path());
    Command::cargo_bin("cece").unwrap()
        .env("HOME", home.path())
        .args(["agent", "list", "--workspace", "nonexistent"])
        .assert()
        .failure()
        .stderr(contains("not found"));
}
```

- [ ] **Step 4: Run agent tests**

```bash
cargo test --test agent_test
```
Expected: passes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: implement agent commands (create, list, delete, switch, logs, watch)"
```

---

## Chunk 6: Cmux Integration

### Task 7: Cmux client

**Files:**
- Modify: `src/cmux/mod.rs`

The Cmux API is available via CLI (`cmux select-workspace`) or Unix socket (`/tmp/cmux.sock`, JSON-RPC). Implement the CLI approach as primary with socket as fallback. If Cmux is not installed, fail gracefully.

- [ ] **Step 1: Implement Cmux module**

Replace `src/cmux/mod.rs`:
```rust
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

fn cmux_available() -> bool {
    Command::new("cmux").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Switch the active Cmux workspace.
pub fn select_workspace(name: &str) -> Result<()> {
    if !cmux_available() {
        anyhow::bail!("cmux is not installed or not in PATH");
    }
    let status = Command::new("cmux")
        .args(["select-workspace", "--workspace", name])
        .status()
        .context("failed to run cmux")?;
    if !status.success() {
        anyhow::bail!("cmux select-workspace failed");
    }
    Ok(())
}

/// Open a new Cmux tab for an agent and return a synthetic session identifier.
pub fn new_agent_tab(workspace: &str, agent_name: &str, working_dir: &Path) -> Result<String> {
    if !cmux_available() {
        anyhow::bail!("cmux is not installed or not in PATH");
    }
    // Attempt to open a new tab in the workspace and run claude
    let status = Command::new("cmux")
        .args([
            "new-tab",
            "--workspace", workspace,
            "--name", agent_name,
            "--",
            "bash", "-c",
            &format!("cd {} && claude", working_dir.display()),
        ])
        .status()
        .context("failed to run cmux new-tab")?;

    if !status.success() {
        anyhow::bail!("cmux new-tab failed");
    }
    Ok(format!("cmux:{}:{}", workspace, agent_name))
}

/// Switch to an existing agent tab in Cmux.
pub fn select_agent_tab(workspace: &str, agent_name: &str) -> Result<()> {
    if !cmux_available() {
        anyhow::bail!("cmux is not installed or not in PATH");
    }
    let status = Command::new("cmux")
        .args(["select-tab", "--workspace", workspace, "--name", agent_name])
        .status()
        .context("failed to run cmux select-tab")?;
    if !status.success() {
        anyhow::bail!("cmux select-tab failed");
    }
    Ok(())
}
```

- [ ] **Step 2: Verify build**

```bash
cargo build
```
Expected: compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: add Cmux integration (workspace and tab management)"
```

---

## Chunk 7: Template, Status, Completions

### Task 8: template commands

**Files:**
- Modify: `src/cli/template.rs`

- [ ] **Step 1: Implement template commands**

Replace `src/cli/template.rs`:
```rust
use anyhow::Result;
use clap::Subcommand;
use comfy_table::{Cell, Table};
use dialoguer::{Input, MultiSelect};

use crate::{db::repo, db::template, open_db};

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// Create a new workspace template
    Create { name: String },
    /// List workspace templates
    List,
    /// Delete a workspace template
    Delete { name: String },
}

pub fn handle_template(cmd: TemplateCommands) -> Result<()> {
    match cmd {
        TemplateCommands::Create { name } => create(&name),
        TemplateCommands::List => list(),
        TemplateCommands::Delete { name } => delete(&name),
    }
}

fn create(name: &str) -> Result<()> {
    let db = open_db()?;

    let branch_template: String = Input::new()
        .with_prompt("Branch name template (e.g. {initials}-{ticket}-{desc})")
        .interact_text()?;

    let known = repo::list(&db)?;
    let repo_paths = if known.is_empty() {
        let path: String = Input::new()
            .with_prompt("Enter a repo path to include (blank to skip)")
            .allow_empty(true)
            .interact_text()?;
        if path.is_empty() { vec![] } else { vec![path] }
    } else {
        let selections = MultiSelect::new()
            .with_prompt("Select repos to include in this template")
            .items(&known)
            .interact()?;
        selections.into_iter().map(|i| known[i].clone()).collect()
    };

    template::create(&db, name, &branch_template, &repo_paths)?;
    println!("Template '{}' created.", name);
    Ok(())
}

fn list() -> Result<()> {
    let db = open_db()?;
    let templates = template::list(&db)?;

    if templates.is_empty() {
        println!("No templates. Use `cece template create <name>` to create one.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(["Name", "Branch Template", "Repos"]);
    for t in &templates {
        table.add_row([
            Cell::new(&t.name),
            Cell::new(&t.branch_template),
            Cell::new(t.repo_paths.join(", ")),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn delete(name: &str) -> Result<()> {
    let db = open_db()?;
    template::delete(&db, name)?;
    println!("Template '{}' deleted.", name);
    Ok(())
}
```

- [ ] **Step 2: Write template test**

Create `tests/template_test.rs`:
```rust
use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn init_cece(home: &std::path::Path) {
    Command::cargo_bin("cece").unwrap()
        .env("HOME", home).env("CECE_NON_INTERACTIVE", "1")
        .arg("init").assert().success();
}

#[test]
fn test_template_list_empty() {
    let home = TempDir::new().unwrap();
    init_cece(home.path());
    Command::cargo_bin("cece").unwrap()
        .env("HOME", home.path())
        .args(["template", "list"])
        .assert()
        .success()
        .stdout(contains("No templates"));
}
```

- [ ] **Step 3: Run template test**

```bash
cargo test --test template_test
```
Expected: passes.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: implement template commands"
```

---

### Task 9: status command

**Files:**
- Modify: `src/cli/status.rs`

- [ ] **Step 1: Implement handle_status()**

Replace `src/cli/status.rs`:
```rust
use anyhow::Result;
use comfy_table::{Cell, Table};

use crate::{db::agent, db::workspace, open_db};

pub fn handle_status() -> Result<()> {
    let db = open_db()?;
    let workspaces = workspace::list(&db)?;

    if workspaces.is_empty() {
        println!("No workspaces. Run `cece ws create <name>` to get started.");
        return Ok(());
    }

    for ws in &workspaces {
        println!("\n Workspace: {}", ws.name);

        let repos = workspace::get_repos(&db, ws.id)?;
        if repos.is_empty() {
            println!("  Repos: (none)");
        } else {
            let mut repo_table = Table::new();
            repo_table.set_header(["Repo", "Branch", "Path"]);
            for r in &repos {
                let repo_name = std::path::Path::new(&r.repo_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                repo_table.add_row([&repo_name, &r.branch, &r.worktree_path]);
            }
            println!("{repo_table}");
        }

        let agents = agent::list(&db, ws.id)?;
        if agents.is_empty() {
            println!("  Agents: (none)");
        } else {
            let mut agent_table = Table::new();
            agent_table.set_header(["Agent", "Session", "Last Request"]);
            for a in &agents {
                agent_table.add_row([
                    Cell::new(&a.name),
                    Cell::new(a.session_id.as_deref().unwrap_or("—")),
                    Cell::new(a.last_request.as_deref().unwrap_or("—")),
                ]);
            }
            println!("{agent_table}");
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Write status test**

Create `tests/status_test.rs`:
```rust
use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn test_status_no_workspaces() {
    let home = TempDir::new().unwrap();
    Command::cargo_bin("cece").unwrap()
        .env("HOME", home.path()).env("CECE_NON_INTERACTIVE", "1")
        .arg("init").assert().success();
    Command::cargo_bin("cece").unwrap()
        .env("HOME", home.path())
        .arg("status")
        .assert()
        .success()
        .stdout(contains("No workspaces"));
}
```

- [ ] **Step 3: Run status test**

```bash
cargo test --test status_test
```
Expected: passes.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: implement status command"
```

---

### Task 10: Shell completions

**Files:**
- Modify: `src/cli/mod.rs`
- Modify: `Cargo.toml`
- Modify: `src/main.rs`

- [ ] **Step 1: Add clap_complete to Cargo.toml**

Add `clap_complete = "4"` to the `[dependencies]` section of `Cargo.toml`. The full dependencies block becomes:

```toml
[dependencies]
anyhow = "1"
thiserror = "2"
clap = { version = "4", features = ["derive", "env"] }
clap_complete = "4"
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dialoguer = "0.11"
comfy-table = "7"
dirs = "5"
```

- [ ] **Step 2: Add completions subcommand to cli/mod.rs**

Replace the entire `src/cli/mod.rs` with the following (adds `clap_complete::Shell` import and `Completions` variant):

```rust
use clap::{Parser, Subcommand};
use clap_complete::Shell;

pub mod agent;
pub mod init;
pub mod status;
pub mod template;
pub mod workspace;

#[derive(Parser)]
#[command(name = "cece", about = "Manage workspaces of Git repos and AI agents", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize cece in your home directory
    Init,
    /// Manage workspaces
    #[command(subcommand, name = "ws")]
    Workspace(workspace::WorkspaceCommands),
    /// Manage agents in the current workspace
    #[command(subcommand)]
    Agent(agent::AgentCommands),
    /// Manage workspace templates
    #[command(subcommand)]
    Template(template::TemplateCommands),
    /// Show status of all workspaces and agents
    Status,
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
}
```

Replace the entire `src/main.rs` match block to add the `Completions` arm:

```rust
mod cli;
mod claude;
mod cmux;
mod db;
mod error;
mod git;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => cli::init::handle_init()?,
        Commands::Workspace(cmd) => cli::workspace::handle_ws(cmd)?,
        Commands::Agent(cmd) => cli::agent::handle_agent(cmd)?,
        Commands::Template(cmd) => cli::template::handle_template(cmd)?,
        Commands::Status => cli::status::handle_status()?,
        Commands::Completions { shell } => {
            use clap::CommandFactory;
            use clap_complete::generate;
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "cece", &mut std::io::stdout());
        }
    }
    Ok(())
}

pub fn cece_dir() -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    Ok(home.join(".cece"))
}

pub fn db_path() -> Result<std::path::PathBuf> {
    Ok(cece_dir()?.join("cece.db"))
}

pub fn open_db() -> Result<db::Database> {
    let path = db_path()?;
    if !path.exists() {
        anyhow::bail!("cece is not initialized — run `cece init` first");
    }
    Ok(db::Database::open(path)?)
}
```

- [ ] **Step 3: Verify completions work**

```bash
cargo run -- completions bash | head -20
```
Expected: outputs bash completion script.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: add shell completion generation"
```

---

## Chunk 8: CI / Release Pipelines

### Task 11: GitHub Actions

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/release.yml`
- Create: `LICENSE`
- Create: `.gitignore`

- [ ] **Step 1: Create .gitignore**

```
/target
# Cargo.lock is committed for binary crates — do not add it here
.DS_Store
*.db
```

- [ ] **Step 2: Create LICENSE (MIT)**

```
MIT License

Copyright (c) 2026 Chris Riddle

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 3: Create .github/workflows/ci.yml**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: fmt
        run: cargo fmt --check
      - name: clippy
        run: cargo clippy -- -D warnings
      - name: test
        run: cargo test
```

- [ ] **Step 4: Create .github/workflows/release.yml**

```yaml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  build:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Build release binary
        run: cargo build --release
      - name: Create archive
        run: |
          cd target/release
          tar -czf cece-${{ github.ref_name }}-macos.tar.gz cece
      - name: Upload to GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: target/release/cece-${{ github.ref_name }}-macos.tar.gz
```

- [ ] **Step 5: Final build and test**

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```
Expected: all pass.

- [ ] **Step 6: Final commit**

```bash
git add -A
git commit -m "chore: add CI/release pipelines, LICENSE, .gitignore"
```

---

## Summary of Chunks

| Chunk | Description | Key Deliverables |
|-------|-------------|-----------------|
| 1 | Project Scaffolding | Cargo.toml, module stubs, DB schema, all CRUD |
| 2 | Init & Config | `cece init`, branch template, Cmux config |
| 3 | Git Helpers | `worktree_add/remove`, branch template expansion |
| 4 | Workspace Commands | `ws create/list/delete/switch` |
| 5 | Agent Commands | `agent create/list/delete/switch/logs/watch` |
| 6 | Cmux Integration | `cmux select-workspace`, `new-tab`, `select-tab` |
| 7 | Templates + Status + Completions | Full feature set |
| 8 | CI / Release | GitHub Actions, LICENSE |
