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
    let mut stmt = db
        .conn()
        .prepare("SELECT path FROM known_repos ORDER BY path")?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    rows.map(|r| r.map_err(Into::into)).collect()
}

#[allow(dead_code)] // used in future task
pub fn remove(db: &Database, path: &str) -> Result<()> {
    db.conn()
        .execute("DELETE FROM known_repos WHERE path = ?1", [path])?;
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
