use crate::db::Database;
use crate::error::{CeceError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    pub id: i64,
    pub name: String,
    pub workspace_id: i64,
    pub working_dir: String,
    /// Cmux surface ID for the agent's terminal pane.
    /// Stored in the `session_id` column for backwards compatibility.
    pub cmux_surface_id: Option<String>,
    /// Claude Code session ID (stem of the .jsonl file in ~/.claude/projects/<encoded>/).
    pub claude_session_id: Option<String>,
    pub last_request: Option<String>,
    pub last_response: Option<String>,
    pub created_at: String,
    pub waiting_for_input: bool,
}

pub fn create(db: &Database, name: &str, workspace_id: i64, working_dir: &str) -> Result<i64> {
    db.conn().execute(
        "INSERT INTO agents (name, workspace_id, working_dir) VALUES (?1, ?2, ?3)",
        (name, workspace_id, working_dir),
    )?;
    Ok(db.conn().last_insert_rowid())
}

pub fn get_by_id(db: &Database, id: i64) -> Result<Option<Agent>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, workspace_id, working_dir, session_id, last_request, created_at, claude_session_id, last_response, waiting_for_input
         FROM agents WHERE id = ?1",
    )?;
    let result = stmt.query_row([id], |r| {
        Ok(Agent {
            id: r.get(0)?,
            name: r.get(1)?,
            workspace_id: r.get(2)?,
            working_dir: r.get(3)?,
            cmux_surface_id: r.get(4)?,
            last_request: r.get(5)?,
            created_at: r.get(6)?,
            claude_session_id: r.get(7)?,
            last_response: r.get(8)?,
            waiting_for_input: r.get::<_, i64>(9)? != 0,
        })
    });
    match result {
        Ok(a) => Ok(Some(a)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(CeceError::Database(e)),
    }
}

pub fn get_by_name(db: &Database, name: &str, workspace_id: i64) -> Result<Agent> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, workspace_id, working_dir, session_id, last_request, created_at, claude_session_id, last_response, waiting_for_input
         FROM agents WHERE name = ?1 AND workspace_id = ?2",
    )?;
    stmt.query_row((name, workspace_id), |r| {
        Ok(Agent {
            id: r.get(0)?,
            name: r.get(1)?,
            workspace_id: r.get(2)?,
            working_dir: r.get(3)?,
            cmux_surface_id: r.get(4)?,
            last_request: r.get(5)?,
            created_at: r.get(6)?,
            claude_session_id: r.get(7)?,
            last_response: r.get(8)?,
            waiting_for_input: r.get::<_, i64>(9)? != 0,
        })
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CeceError::AgentNotFound(name.to_string()),
        other => CeceError::Database(other),
    })
}

pub fn list(db: &Database, workspace_id: i64) -> Result<Vec<Agent>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, workspace_id, working_dir, session_id, last_request, created_at, claude_session_id, last_response, waiting_for_input
         FROM agents WHERE workspace_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map([workspace_id], |r| {
        Ok(Agent {
            id: r.get(0)?,
            name: r.get(1)?,
            workspace_id: r.get(2)?,
            working_dir: r.get(3)?,
            cmux_surface_id: r.get(4)?,
            last_request: r.get(5)?,
            created_at: r.get(6)?,
            claude_session_id: r.get(7)?,
            last_response: r.get(8)?,
            waiting_for_input: r.get::<_, i64>(9)? != 0,
        })
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

/// Find an agent by its stored Claude Code session ID.
pub fn find_by_claude_session_id(db: &Database, session_id: &str) -> Result<Option<Agent>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, workspace_id, working_dir, session_id, last_request, created_at, claude_session_id, last_response, waiting_for_input
         FROM agents WHERE claude_session_id = ?1 LIMIT 1",
    )?;
    let result = stmt.query_row([session_id], |r| {
        Ok(Agent {
            id: r.get(0)?,
            name: r.get(1)?,
            workspace_id: r.get(2)?,
            working_dir: r.get(3)?,
            cmux_surface_id: r.get(4)?,
            last_request: r.get(5)?,
            created_at: r.get(6)?,
            claude_session_id: r.get(7)?,
            last_response: r.get(8)?,
            waiting_for_input: r.get::<_, i64>(9)? != 0,
        })
    });
    match result {
        Ok(a) => Ok(Some(a)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(CeceError::Database(e)),
    }
}

/// Find an agent by exact working directory path. Returns `None` if no agent matches.
pub fn find_by_working_dir(db: &Database, working_dir: &str) -> Result<Option<Agent>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, workspace_id, working_dir, session_id, last_request, created_at, claude_session_id, last_response, waiting_for_input
         FROM agents WHERE working_dir = ?1 LIMIT 1",
    )?;
    let result = stmt.query_row([working_dir], |r| {
        Ok(Agent {
            id: r.get(0)?,
            name: r.get(1)?,
            workspace_id: r.get(2)?,
            working_dir: r.get(3)?,
            cmux_surface_id: r.get(4)?,
            last_request: r.get(5)?,
            created_at: r.get(6)?,
            claude_session_id: r.get(7)?,
            last_response: r.get(8)?,
            waiting_for_input: r.get::<_, i64>(9)? != 0,
        })
    });
    match result {
        Ok(a) => Ok(Some(a)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(CeceError::Database(e)),
    }
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

/// Update the cmux surface ID (and optionally last_request) for an agent.
pub fn update_cmux_surface(
    db: &Database,
    id: i64,
    surface_id: &str,
    last_request: Option<&str>,
) -> Result<()> {
    db.conn().execute(
        "UPDATE agents SET session_id = ?1, last_request = ?2 WHERE id = ?3",
        (surface_id, last_request, id),
    )?;
    Ok(())
}

/// Update the stored Claude Code session ID for an agent.
pub fn update_claude_session(db: &Database, id: i64, session_id: &str) -> Result<()> {
    db.conn().execute(
        "UPDATE agents SET claude_session_id = ?1 WHERE id = ?2",
        (session_id, id),
    )?;
    Ok(())
}

/// Update the last user request (clipped).
pub fn update_last_request(db: &Database, id: i64, request: &str) -> Result<()> {
    db.conn().execute(
        "UPDATE agents SET last_request = ?1 WHERE id = ?2",
        (request, id),
    )?;
    Ok(())
}

/// Update the last Claude response (clipped).
pub fn update_last_response(db: &Database, id: i64, response: &str) -> Result<()> {
    db.conn().execute(
        "UPDATE agents SET last_response = ?1 WHERE id = ?2",
        (response, id),
    )?;
    Ok(())
}

/// Set or clear the waiting-for-input flag for an agent.
pub fn set_waiting_for_input(db: &Database, id: i64, waiting: bool) -> Result<()> {
    db.conn().execute(
        "UPDATE agents SET waiting_for_input = ?1 WHERE id = ?2",
        (waiting as i64, id),
    )?;
    Ok(())
}

/// Return all agents (across all workspaces) that are waiting for user input,
/// paired with their workspace name.
pub fn list_waiting(db: &Database) -> Result<Vec<(Agent, String)>> {
    let mut stmt = db.conn().prepare(
        "SELECT a.id, a.name, a.workspace_id, a.working_dir, a.session_id,
                a.last_request, a.created_at, a.claude_session_id, a.last_response,
                a.waiting_for_input, w.name
         FROM agents a
         JOIN workspaces w ON w.id = a.workspace_id
         WHERE a.waiting_for_input = 1
         ORDER BY w.name, a.name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            Agent {
                id: r.get(0)?,
                name: r.get(1)?,
                workspace_id: r.get(2)?,
                working_dir: r.get(3)?,
                cmux_surface_id: r.get(4)?,
                last_request: r.get(5)?,
                created_at: r.get(6)?,
                claude_session_id: r.get(7)?,
                last_response: r.get(8)?,
                waiting_for_input: r.get::<_, i64>(9)? != 0,
            },
            r.get::<_, String>(10)?,
        ))
    })?;
    rows.map(|r| r.map_err(Into::into)).collect()
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
        assert_eq!(agent.cmux_surface_id, None);
        assert_eq!(agent.claude_session_id, None);
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
    fn test_update_cmux_surface() {
        let (db, ws_id) = setup();
        let id = create(&db, "a1", ws_id, "/cece/ws").unwrap();
        update_cmux_surface(&db, id, "surface-abc", Some("Fix the auth bug")).unwrap();
        let agent = get_by_name(&db, "a1", ws_id).unwrap();
        assert_eq!(agent.cmux_surface_id, Some("surface-abc".to_string()));
        assert_eq!(agent.last_request, Some("Fix the auth bug".to_string()));
    }

    #[test]
    fn test_update_claude_session() {
        let (db, ws_id) = setup();
        let id = create(&db, "a1", ws_id, "/cece/ws").unwrap();
        update_claude_session(&db, id, "abc123def456").unwrap();
        let agent = get_by_name(&db, "a1", ws_id).unwrap();
        assert_eq!(agent.claude_session_id, Some("abc123def456".to_string()));
    }

    #[test]
    fn test_waiting_for_input_defaults_false() {
        let (db, ws_id) = setup();
        let id = create(&db, "a1", ws_id, "/cece/ws").unwrap();
        let agent = get_by_id(&db, id).unwrap().unwrap();
        assert!(!agent.waiting_for_input);
    }

    #[test]
    fn test_set_waiting_for_input() {
        let (db, ws_id) = setup();
        let id = create(&db, "a1", ws_id, "/cece/ws").unwrap();
        set_waiting_for_input(&db, id, true).unwrap();
        let agent = get_by_id(&db, id).unwrap().unwrap();
        assert!(agent.waiting_for_input);

        set_waiting_for_input(&db, id, false).unwrap();
        let agent = get_by_id(&db, id).unwrap().unwrap();
        assert!(!agent.waiting_for_input);
    }

    #[test]
    fn test_list_waiting() {
        let db = Database::open_in_memory().unwrap();
        let ws_id = workspace::create(&db, "ws").unwrap();
        let id1 = create(&db, "a1", ws_id, "/cece/ws").unwrap();
        let _id2 = create(&db, "a2", ws_id, "/cece/ws").unwrap();

        set_waiting_for_input(&db, id1, true).unwrap();
        // a2 stays false

        let waiting = list_waiting(&db).unwrap();
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].0.name, "a1");
        assert_eq!(waiting[0].1, "ws");

        set_waiting_for_input(&db, id1, false).unwrap();
        assert!(list_waiting(&db).unwrap().is_empty());
    }
}
