use crate::db::Database;
use crate::error::{CeceError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub id: i64,
    pub name: String,
    pub branch_template: String,
    pub repo_paths: Vec<String>,
}

pub fn create(
    db: &Database,
    name: &str,
    branch_template: &str,
    repo_paths: &[String],
) -> Result<i64> {
    let repo_paths_json = serde_json::to_string(repo_paths).expect("serialization is infallible");
    db.conn().execute(
        "INSERT INTO templates (name, branch_template, repo_paths) VALUES (?1, ?2, ?3)",
        (name, branch_template, &repo_paths_json),
    )?;
    Ok(db.conn().last_insert_rowid())
}

pub fn get_by_name(db: &Database, name: &str) -> Result<Template> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, name, branch_template, repo_paths FROM templates WHERE name = ?1")?;
    stmt.query_row([name], |r| {
        let repo_paths_json: String = r.get(3)?;
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            repo_paths_json,
        ))
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CeceError::TemplateNotFound(name.to_string()),
        other => CeceError::Database(other),
    })
    .and_then(|(id, tname, branch_template, json)| {
        let repo_paths = serde_json::from_str(&json)
            .map_err(|e| CeceError::Git(format!("invalid template data: {e}")))?;
        Ok(Template {
            id,
            name: tname,
            branch_template,
            repo_paths,
        })
    })
}

pub fn list(db: &Database) -> Result<Vec<Template>> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, name, branch_template, repo_paths FROM templates ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        let json: String = r.get(3)?;
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            json,
        ))
    })?;
    rows.map(|r| {
        let (id, name, branch_template, json) = r?;
        let repo_paths = serde_json::from_str(&json)
            .map_err(|e| CeceError::Git(format!("invalid template data: {e}")))?;
        Ok(Template {
            id,
            name,
            branch_template,
            repo_paths,
        })
    })
    .collect()
}

pub fn delete(db: &Database, name: &str) -> Result<()> {
    let rows = db
        .conn()
        .execute("DELETE FROM templates WHERE name = ?1", [name])?;
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
