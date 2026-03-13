use anyhow::Result;
use clap::Subcommand;
use comfy_table::{Cell, Table};
use std::path::PathBuf;

use anyhow::Context;

use crate::{db::agent, db::config, db::workspace, open_db};

#[derive(Subcommand)]
pub enum AgentCommands {
    /// Create a new agent in the current workspace
    Create {
        name: String,
        /// Workspace to create the agent in
        #[arg(long)]
        workspace: String,
        /// Working directory (defaults to workspace dir)
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// List agents in a workspace
    List {
        /// Workspace name
        #[arg(long)]
        workspace: String,
    },
    /// Delete an agent
    Delete {
        name: String,
        #[arg(long)]
        workspace: String,
    },
    /// Switch to (open/focus) an agent
    Switch {
        name: String,
        #[arg(long)]
        workspace: String,
    },
    /// Show session history for an agent
    Logs {
        name: String,
        #[arg(long)]
        workspace: String,
    },
    /// Wait until an agent is idle
    Watch {
        name: String,
        #[arg(long)]
        workspace: String,
    },
}

pub fn handle_agent(cmd: AgentCommands) -> Result<()> {
    match cmd {
        AgentCommands::Create {
            name,
            workspace,
            dir,
        } => create(&name, &workspace, dir),
        AgentCommands::List { workspace } => list(&workspace),
        AgentCommands::Delete { name, workspace } => delete(&name, &workspace),
        AgentCommands::Switch { name, workspace } => switch(&name, &workspace),
        AgentCommands::Logs { name, workspace } => logs(&name, &workspace),
        AgentCommands::Watch { name, workspace } => watch(&name, &workspace),
    }
}

fn create(name: &str, workspace_name: &str, dir_override: Option<PathBuf>) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    let ws_dir = crate::cece_dir()?.join("workspaces").join(workspace_name);
    let working_dir = dir_override.unwrap_or(ws_dir);

    let id = agent::create(&db, name, ws.id, &working_dir.to_string_lossy())?;
    println!(
        "Agent '{}' created in workspace '{}'.",
        name, workspace_name
    );

    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");
    if cmux_enabled {
        let cmux_id = crate::cli::workspace::ensure_cmux_workspace(&db, &ws, workspace_name)?;
        let session_id = crate::cmux::new_agent_tab(&cmux_id, name, &working_dir)?;
        agent::update_session(&db, id, &session_id, None)?;
        println!("Opened in Cmux tab.");
    } else {
        println!("Launch Claude Code manually:");
        println!("  cd {} && claude", working_dir.display());
    }
    Ok(())
}

fn list(workspace_name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    let agents = agent::list(&db, ws.id)?;

    if agents.is_empty() {
        println!("No agents in workspace '{workspace_name}'.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(["Name", "Working Dir", "Session ID", "Last Request"]);
    for a in &agents {
        table.add_row([
            Cell::new(&a.name),
            Cell::new(&a.working_dir),
            Cell::new(a.session_id.as_deref().unwrap_or("—")),
            Cell::new(a.last_request.as_deref().unwrap_or("—")),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn delete(name: &str, workspace_name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    agent::delete(&db, name, ws.id)?;
    println!("Agent '{}' deleted.", name);
    Ok(())
}

fn switch(name: &str, workspace_name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    let a = agent::get_by_name(&db, name, ws.id)?;

    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");
    if cmux_enabled {
        let surface_id = a
            .session_id
            .as_deref()
            .with_context(|| format!("agent '{name}' has no cmux surface — was it created with cmux enabled?"))?;
        crate::cmux::select_agent_tab(surface_id)?;
        println!("Switched to agent '{}' in Cmux.", name);
    } else {
        println!("{}", a.working_dir);
        eprintln!("(Tip: cd to that directory and run `claude --continue`)");
    }
    Ok(())
}

fn logs(name: &str, workspace_name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
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

fn watch(name: &str, workspace_name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, workspace_name)?;
    let a = agent::get_by_name(&db, name, ws.id)?;

    println!("Watching agent '{name}'... (Ctrl+C to stop)");
    crate::claude::wait_until_idle(&a.working_dir)?;
    println!("Agent '{name}' is idle.");
    Ok(())
}
