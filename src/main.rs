use anyhow::Result;
use cece::cli::{Cli, Commands};
use cece::open_db;
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => cece::cli::init::handle_init()?,
        Commands::Workspace(cmd) => cece::cli::workspace::handle_ws(cmd)?,
        Commands::Agent(cmd) => cece::cli::agent::handle_agent(cmd)?,
        Commands::Template(cmd) => cece::cli::template::handle_template(cmd)?,
        Commands::List => cece::cli::list::handle_list()?,
        Commands::Status => cece::cli::status::handle_status()?,
        Commands::Idea => open_editor("idea")?,
        Commands::Zed => open_editor("zed")?,
        Commands::Code => open_editor("code")?,
        Commands::Cursor => open_editor("cursor")?,
        Commands::Hook(cmd) => {
            cece::cli::hook::handle_hook(cmd);
        }
        Commands::Completions { shell } => {
            use clap::CommandFactory;
            use clap_complete::generate;
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "cece", &mut std::io::stdout());
        }
    }
    Ok(())
}

fn open_editor(cmd: &str) -> Result<()> {
    let db = open_db()?;
    let cwd = std::env::current_dir()?;

    let ws = cece::db::workspace::find_by_worktree(&db, &cwd)?.ok_or_else(|| {
        anyhow::anyhow!("not inside a cece worktree — run this from inside a workspace directory")
    })?;

    let repos = cece::db::workspace::get_repos(&db, ws.id)?;
    let worktree_path = repos
        .into_iter()
        .find(|r| cwd.starts_with(&r.worktree_path))
        .map(|r| std::path::PathBuf::from(r.worktree_path))
        .unwrap_or(cwd);

    let status = std::process::Command::new(cmd)
        .arg(&worktree_path)
        .status()
        .map_err(|e| {
            anyhow::anyhow!("failed to run `{cmd}`: {e} — is it installed and in PATH?")
        })?;

    if !status.success() {
        anyhow::bail!("`{cmd}` exited with status {status}");
    }
    Ok(())
}
