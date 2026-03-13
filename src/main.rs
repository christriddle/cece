mod claude;
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
