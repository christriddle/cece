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
        Commands::Idea { workspace } => open_editor("idea", workspace)?,
        Commands::Zed { workspace } => open_editor("zed", workspace)?,
        Commands::Code { workspace } => open_editor("code", workspace)?,
        Commands::Cursor { workspace } => open_editor("cursor", workspace)?,
        Commands::Hook(cmd) => {
            cece::cli::hook::handle_hook(cmd);
        }
        Commands::Complete(cmd) => handle_complete(cmd)?,
        Commands::Completions { shell } => handle_completions(shell),
    }
    Ok(())
}

fn handle_completions(shell: clap_complete::Shell) {
    use clap_complete::Shell;
    match shell {
        Shell::Zsh => print!("{}", include_str!("completions/cece.zsh")),
        _ => {
            use clap::CommandFactory;
            use clap_complete::generate;
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "cece", &mut std::io::stdout());
        }
    }
}

fn handle_complete(cmd: cece::cli::CompleteCommands) -> Result<()> {
    let db = open_db()?;
    match cmd {
        cece::cli::CompleteCommands::Workspaces => {
            for ws in cece::db::workspace::list(&db)? {
                println!("{}", ws.name);
            }
        }
        cece::cli::CompleteCommands::Agents { workspace } => {
            let ws = cece::db::workspace::get_by_name(&db, &workspace)?;
            for a in cece::db::agent::list(&db, ws.id)? {
                println!("{}", a.name);
            }
        }
    }
    Ok(())
}

fn open_editor(cmd: &str, workspace: Option<String>) -> Result<()> {
    let db = open_db()?;

    let target = if let Some(name) = workspace {
        // Explicit workspace: open the workspace root directory.
        let ws = cece::db::workspace::get_by_name(&db, &name)?;
        let ws_dir = cece::cece_dir()?.join("workspaces").join(&ws.name);
        let repos = cece::db::workspace::get_repos(&db, ws.id)?;
        // Single repo: open the worktree directly. Multi-repo: open the workspace root.
        if repos.len() == 1 {
            std::path::PathBuf::from(&repos[0].worktree_path)
        } else {
            ws_dir
        }
    } else {
        // Infer from cwd.
        let cwd = std::env::current_dir()?;
        let ws = cece::db::workspace::find_by_worktree(&db, &cwd)?.ok_or_else(|| {
            anyhow::anyhow!("not inside a cece worktree — pass a workspace name or cd into one")
        })?;
        let repos = cece::db::workspace::get_repos(&db, ws.id)?;
        repos
            .into_iter()
            .find(|r| cwd.starts_with(&r.worktree_path))
            .map(|r| std::path::PathBuf::from(r.worktree_path))
            .unwrap_or(cwd)
    };

    let status = std::process::Command::new(cmd)
        .arg(&target)
        .status()
        .map_err(|e| {
            anyhow::anyhow!("failed to run `{cmd}`: {e} — is it installed and in PATH?")
        })?;

    if !status.success() {
        anyhow::bail!("`{cmd}` exited with status {status}");
    }
    Ok(())
}
