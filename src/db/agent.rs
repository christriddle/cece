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
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CeceError::AgentNotFound(name.to_string()),
        other => CeceError::Database(other),
    })
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

pub fn update_session(
    db: &Database,
    id: i64,
    session_id: &str,
    last_request: Option<&str>,
) -> Result<()> {
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
