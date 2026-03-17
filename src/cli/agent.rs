use anyhow::{Context, Result};
use clap::Subcommand;
use comfy_table::{Cell, Color};
use std::path::PathBuf;

use super::styled_table;

use crate::{db::agent, db::config, db::workspace, open_db};

#[derive(Subcommand)]
pub enum AgentCommands {
    /// Create a new agent in the current workspace
    Create {
        name: String,
        /// Workspace name (inferred from cwd if omitted)
        #[arg(long)]
        workspace: Option<String>,
        /// Working directory (defaults to workspace dir)
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// List agents in a workspace
    List {
        /// Workspace name (inferred from cwd if omitted)
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Delete an agent
    Delete {
        name: String,
        /// Workspace name (inferred from cwd if omitted)
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Switch to (open/focus) an agent
    Switch {
        name: String,
        /// Workspace name (inferred from cwd if omitted)
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Show session history for an agent
    Logs {
        name: String,
        /// Workspace name (inferred from cwd if omitted)
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Wait until an agent is idle
    Watch {
        name: String,
        /// Workspace name (inferred from cwd if omitted)
        #[arg(long)]
        workspace: Option<String>,
    },
}

pub fn handle_agent(cmd: AgentCommands) -> Result<()> {
    match cmd {
        AgentCommands::Create {
            name,
            workspace,
            dir,
        } => create(&name, workspace, dir),
        AgentCommands::List { workspace } => list(workspace),
        AgentCommands::Delete { name, workspace } => delete(&name, workspace),
        AgentCommands::Switch { name, workspace } => switch(&name, workspace),
        AgentCommands::Logs { name, workspace } => logs(&name, workspace),
        AgentCommands::Watch { name, workspace } => watch(&name, workspace),
    }
}

/// Resolve workspace name: use the explicit value if given, otherwise detect from cwd.
fn resolve_workspace(db: &crate::db::Database, explicit: Option<String>) -> Result<String> {
    if let Some(name) = explicit {
        return Ok(name);
    }
    let cwd = std::env::current_dir().context("cannot determine current directory")?;
    workspace::find_by_worktree(db, &cwd)?
        .map(|ws| ws.name)
        .context("cannot infer workspace from current directory — use --workspace")
}

fn create(name: &str, workspace_arg: Option<String>, dir_override: Option<PathBuf>) -> Result<()> {
    let db = open_db()?;
    let workspace_name = resolve_workspace(&db, workspace_arg)?;
    let ws = workspace::get_by_name(&db, &workspace_name)?;
    let ws_dir = crate::cece_dir()?.join("workspaces").join(&workspace_name);
    let working_dir = dir_override.unwrap_or_else(|| ws_dir.clone());

    let id = agent::create(&db, name, ws.id, &working_dir.to_string_lossy())?;
    println!(
        "Agent '{}' created in workspace '{}'.",
        name, workspace_name
    );

    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");
    if cmux_enabled {
        let repos = workspace::get_repos(&db, ws.id)?;
        let ws_start_dir = if repos.len() == 1 {
            std::path::PathBuf::from(&repos[0].worktree_path)
        } else {
            ws_dir
        };
        let cmux_id =
            crate::cli::workspace::ensure_cmux_workspace(&db, &ws, &workspace_name, &ws_start_dir)?;
        let surface_id = ws.cmux_surface_id.as_deref().with_context(|| {
            format!("workspace '{workspace_name}' has no command-center surface — try re-creating it with cmux enabled")
        })?;
        let new_surface_id =
            crate::cmux::new_agent_tab(&cmux_id, surface_id, name, id, &working_dir, None)?;
        agent::update_cmux_surface(&db, id, &new_surface_id, None)?;
        println!("Opened in Cmux tab.");
    } else {
        println!("Launch Claude Code manually:");
        println!("  cd {} && claude", working_dir.display());
    }
    Ok(())
}

fn list(workspace_arg: Option<String>) -> Result<()> {
    let db = open_db()?;
    let workspace_name = resolve_workspace(&db, workspace_arg)?;
    let ws = workspace::get_by_name(&db, &workspace_name)?;
    let agents = agent::list(&db, ws.id)?;

    if agents.is_empty() {
        println!("No agents in workspace '{workspace_name}'.");
        return Ok(());
    }

    let mut table = styled_table(&["Name", "Last Request", "Last Response"]);
    for a in &agents {
        table.add_row([
            Cell::new(&a.name).fg(Color::Green),
            Cell::new(a.last_request.as_deref().unwrap_or("—")),
            Cell::new(a.last_response.as_deref().unwrap_or("—")),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn delete(name: &str, workspace_arg: Option<String>) -> Result<()> {
    let db = open_db()?;
    let workspace_name = resolve_workspace(&db, workspace_arg)?;
    let ws = workspace::get_by_name(&db, &workspace_name)?;
    let a = agent::get_by_name(&db, name, ws.id)?;

    if let Some(surface_id) = a.cmux_surface_id.as_deref() {
        let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");
        if cmux_enabled {
            crate::cmux::close_surface(surface_id);
        }
    }

    agent::delete(&db, name, ws.id)?;
    println!("Agent '{}' deleted.", name);
    Ok(())
}

fn switch(name: &str, workspace_arg: Option<String>) -> Result<()> {
    let db = open_db()?;
    let workspace_name = resolve_workspace(&db, workspace_arg)?;
    let ws = workspace::get_by_name(&db, &workspace_name)?;
    let a = agent::get_by_name(&db, name, ws.id)?;

    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");
    if cmux_enabled {
        let surface_id = a.cmux_surface_id.as_deref().with_context(|| {
            format!("agent '{name}' has no cmux surface — was it created with cmux enabled?")
        })?;

        // Try to focus the existing surface. If it's gone, recreate it and resume the session.
        match crate::cmux::select_agent_tab(surface_id) {
            Ok(()) => {}
            Err(e) if format!("{e:#}").contains("not_found") => {
                eprintln!("Surface no longer exists, reopening agent...");
                let ws_dir = crate::cece_dir()?.join("workspaces").join(&workspace_name);
                let repos = workspace::get_repos(&db, ws.id)?;
                let ws_start_dir = if repos.len() == 1 {
                    std::path::PathBuf::from(&repos[0].worktree_path)
                } else {
                    ws_dir
                };
                let cmux_id = crate::cli::workspace::ensure_cmux_workspace(
                    &db,
                    &ws,
                    &workspace_name,
                    &ws_start_dir,
                )?;
                let cc_surface = ws.cmux_surface_id.as_deref().with_context(|| {
                    format!("workspace '{workspace_name}' has no command-center surface")
                })?;
                let new_surface_id = crate::cmux::new_agent_tab(
                    &cmux_id,
                    cc_surface,
                    name,
                    a.id,
                    std::path::Path::new(&a.working_dir),
                    a.claude_session_id.as_deref(),
                )?;
                agent::update_cmux_surface(&db, a.id, &new_surface_id, a.last_request.as_deref())?;
            }
            Err(e) => return Err(e),
        }
        println!("Switched to agent '{}' in Cmux.", name);
    } else {
        println!("{}", a.working_dir);
        eprintln!("(Tip: cd to that directory and run `claude --continue`)");
    }
    Ok(())
}

fn logs(name: &str, workspace_arg: Option<String>) -> Result<()> {
    let db = open_db()?;
    let workspace_name = resolve_workspace(&db, workspace_arg)?;
    let ws = workspace::get_by_name(&db, &workspace_name)?;
    let a = agent::get_by_name(&db, name, ws.id)?;

    let logs = crate::claude::read_session_logs(&a.working_dir)?;
    if logs.is_empty() {
        println!("No session logs found for agent '{name}'.");
    } else {
        for entry in &logs {
            println!("{entry}");
        }
    }
    Ok(())
}

fn watch(name: &str, workspace_arg: Option<String>) -> Result<()> {
    let db = open_db()?;
    let workspace_name = resolve_workspace(&db, workspace_arg)?;
    let ws = workspace::get_by_name(&db, &workspace_name)?;
    let a = agent::get_by_name(&db, name, ws.id)?;

    println!("Watching agent '{name}'... (Ctrl+C to stop)");
    crate::claude::wait_until_idle(&a.working_dir)?;
    println!("Agent '{name}' is idle.");
    Ok(())
}
