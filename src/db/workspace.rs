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
    db.conn()
        .execute("INSERT INTO workspaces (name) VALUES (?1)", [name])?;
    Ok(db.conn().last_insert_rowid())
}

pub fn get_by_name(db: &Database, name: &str) -> Result<Workspace> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, name, created_at FROM workspaces WHERE name = ?1")?;
    stmt.query_row([name], |r| {
        Ok(Workspace {
            id: r.get(0)?,
            name: r.get(1)?,
            created_at: r.get(2)?,
        })
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CeceError::WorkspaceNotFound(name.to_string()),
        other => CeceError::Database(other),
    })
}

pub fn list(db: &Database) -> Result<Vec<Workspace>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, name, created_at FROM workspaces ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        Ok(Workspace {
            id: r.get(0)?,
            name: r.get(1)?,
            created_at: r.get(2)?,
        })
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

pub fn delete(db: &Database, name: &str) -> Result<()> {
    let rows = db
        .conn()
        .execute("DELETE FROM workspaces WHERE name = ?1", [name])?;
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
