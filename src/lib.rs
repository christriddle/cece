pub mod claude;
pub mod cli;
pub mod cmux;
pub mod db;
pub mod error;
pub mod git;

use anyhow::Result;

pub fn cece_dir() -> Result<std::path::PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
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
