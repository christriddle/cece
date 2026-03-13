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
