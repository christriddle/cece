use crate::db::Database;
use crate::error::{CeceError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub cmux_workspace_id: Option<String>,
    pub cmux_surface_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceRepo {
    pub id: i64,
    pub workspace_id: i64,
    pub repo_path: String,
    pub branch: String,
    pub worktree_path: String,
    /// Whether this branch was freshly created for this workspace (vs an existing branch).
    /// Only freshly-created branches are deleted when the workspace is removed.
    pub branch_new: bool,
}

pub fn create(db: &Database, name: &str) -> Result<i64> {
    db.conn()
        .execute("INSERT INTO workspaces (name) VALUES (?1)", [name])?;
    Ok(db.conn().last_insert_rowid())
}

pub fn get_by_name(db: &Database, name: &str) -> Result<Workspace> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, cmux_workspace_id, cmux_surface_id, created_at FROM workspaces WHERE name = ?1",
    )?;
    stmt.query_row([name], |r| {
        Ok(Workspace {
            id: r.get(0)?,
            name: r.get(1)?,
            cmux_workspace_id: r.get(2)?,
            cmux_surface_id: r.get(3)?,
            created_at: r.get(4)?,
        })
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CeceError::WorkspaceNotFound(name.to_string()),
        other => CeceError::Database(other),
    })
}

pub fn list(db: &Database) -> Result<Vec<Workspace>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, cmux_workspace_id, cmux_surface_id, created_at FROM workspaces ORDER BY name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Workspace {
            id: r.get(0)?,
            name: r.get(1)?,
            cmux_workspace_id: r.get(2)?,
            cmux_surface_id: r.get(3)?,
            created_at: r.get(4)?,
        })
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

pub fn set_cmux_id(db: &Database, workspace_id: i64, cmux_id: &str) -> Result<()> {
    db.conn().execute(
        "UPDATE workspaces SET cmux_workspace_id = ?1 WHERE id = ?2",
        (cmux_id, workspace_id),
    )?;
    Ok(())
}

pub fn set_cmux_surface_id(db: &Database, workspace_id: i64, surface_id: &str) -> Result<()> {
    db.conn().execute(
        "UPDATE workspaces SET cmux_surface_id = ?1 WHERE id = ?2",
        (surface_id, workspace_id),
    )?;
    Ok(())
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

/// Find the workspace whose worktree contains the given path (exact or ancestor match).
pub fn find_by_worktree(db: &Database, cwd: &std::path::Path) -> Result<Option<Workspace>> {
    let mut stmt = db.conn().prepare(
        "SELECT w.id, w.name, w.cmux_workspace_id, w.cmux_surface_id, w.created_at
         FROM workspaces w
         JOIN workspace_repos wr ON wr.workspace_id = w.id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Workspace {
            id: r.get(0)?,
            name: r.get(1)?,
            cmux_workspace_id: r.get(2)?,
            cmux_surface_id: r.get(3)?,
            created_at: r.get(4)?,
        })
    })?;
    // Return the first workspace where cwd is inside a worktree path OR
    // a worktree path is inside cwd (e.g. cwd is the workspace root directory).
    for row in rows {
        let ws = row?;
        let repos = get_repos(db, ws.id)?;
        for repo in &repos {
            let wt = std::path::Path::new(&repo.worktree_path);
            if cwd.starts_with(wt) || wt.starts_with(cwd) {
                return Ok(Some(ws));
            }
        }
    }
    Ok(None)
}

pub fn add_repo(
    db: &Database,
    workspace_id: i64,
    repo_path: &str,
    branch: &str,
    worktree_path: &str,
    branch_new: bool,
) -> Result<()> {
    db.conn().execute(
        "INSERT INTO workspace_repos (workspace_id, repo_path, branch, worktree_path, branch_new)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (
            workspace_id,
            repo_path,
            branch,
            worktree_path,
            branch_new as i64,
        ),
    )?;
    Ok(())
}

pub fn remove_repo(db: &Database, workspace_id: i64, repo_path: &str) -> Result<WorkspaceRepo> {
    let mut stmt = db.conn().prepare(
        "SELECT id, workspace_id, repo_path, branch, worktree_path, branch_new
         FROM workspace_repos WHERE workspace_id = ?1 AND repo_path = ?2",
    )?;
    let record = stmt
        .query_row((workspace_id, repo_path), |r| {
            Ok(WorkspaceRepo {
                id: r.get(0)?,
                workspace_id: r.get(1)?,
                repo_path: r.get(2)?,
                branch: r.get(3)?,
                worktree_path: r.get(4)?,
                branch_new: r.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                CeceError::RepoNotFoundInWorkspace(repo_path.to_string())
            }
            other => CeceError::Database(other),
        })?;

    db.conn()
        .execute("DELETE FROM workspace_repos WHERE id = ?1", [record.id])?;

    Ok(record)
}

pub fn get_repos(db: &Database, workspace_id: i64) -> Result<Vec<WorkspaceRepo>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, workspace_id, repo_path, branch, worktree_path, branch_new
         FROM workspace_repos WHERE workspace_id = ?1",
    )?;
    let rows = stmt.query_map([workspace_id], |r| {
        Ok(WorkspaceRepo {
            id: r.get(0)?,
            workspace_id: r.get(1)?,
            repo_path: r.get(2)?,
            branch: r.get(3)?,
            worktree_path: r.get(4)?,
            branch_new: r.get::<_, i64>(5)? != 0,
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
    fn test_find_by_worktree_inside_repo() {
        let db = Database::open_in_memory().unwrap();
        let ws_id = create(&db, "multi").unwrap();
        add_repo(
            &db,
            ws_id,
            "/repos/frontend",
            "main",
            "/cece/workspaces/multi/frontend",
            false,
        )
        .unwrap();
        add_repo(
            &db,
            ws_id,
            "/repos/backend",
            "main",
            "/cece/workspaces/multi/backend",
            false,
        )
        .unwrap();

        // Inside a worktree — should match.
        let found =
            find_by_worktree(&db, std::path::Path::new("/cece/workspaces/multi/frontend")).unwrap();
        assert_eq!(found.unwrap().name, "multi");

        // Inside a subdirectory of a worktree — should match.
        let found = find_by_worktree(
            &db,
            std::path::Path::new("/cece/workspaces/multi/backend/src"),
        )
        .unwrap();
        assert_eq!(found.unwrap().name, "multi");

        // At the workspace root (parent of worktrees) — should match.
        let found = find_by_worktree(&db, std::path::Path::new("/cece/workspaces/multi")).unwrap();
        assert_eq!(found.unwrap().name, "multi");

        // Completely unrelated path — no match.
        let found = find_by_worktree(&db, std::path::Path::new("/other/path")).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_remove_repo() {
        let db = Database::open_in_memory().unwrap();
        let ws_id = create(&db, "ws").unwrap();
        add_repo(
            &db,
            ws_id,
            "/repos/frontend",
            "main",
            "/cece/ws/frontend",
            false,
        )
        .unwrap();
        add_repo(
            &db,
            ws_id,
            "/repos/backend",
            "main",
            "/cece/ws/backend",
            false,
        )
        .unwrap();

        let removed = remove_repo(&db, ws_id, "/repos/frontend").unwrap();
        assert_eq!(removed.repo_path, "/repos/frontend");

        let remaining = get_repos(&db, ws_id).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].repo_path, "/repos/backend");
    }

    #[test]
    fn test_remove_repo_nonexistent_errors() {
        let db = Database::open_in_memory().unwrap();
        let ws_id = create(&db, "ws").unwrap();
        let result = remove_repo(&db, ws_id, "/repos/ghost");
        assert!(matches!(result, Err(CeceError::RepoNotFoundInWorkspace(_))));
    }

    #[test]
    fn test_add_and_get_repos() {
        let db = Database::open_in_memory().unwrap();
        let ws_id = create(&db, "ws").unwrap();
        add_repo(
            &db,
            ws_id,
            "/repos/frontend",
            "main",
            "/cece/ws/frontend",
            false,
        )
        .unwrap();
        let repos = get_repos(&db, ws_id).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].branch, "main");
    }
}
