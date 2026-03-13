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
    let mut stmt = db
        .conn()
        .prepare("SELECT value FROM config WHERE key = ?1")?;
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
