use anyhow::Result;
use comfy_table::{Cell, Table};

use crate::{db::agent, db::workspace, open_db};

pub fn handle_status() -> Result<()> {
    let db = open_db()?;
    let workspaces = workspace::list(&db)?;

    if workspaces.is_empty() {
        println!("No workspaces. Run `cece ws create <name>` to get started.");
        return Ok(());
    }

    for ws in &workspaces {
        println!("\n Workspace: {}", ws.name);

        let repos = workspace::get_repos(&db, ws.id)?;
        if repos.is_empty() {
            println!("  Repos: (none)");
        } else {
            let mut repo_table = Table::new();
            repo_table.set_header(["Repo", "Branch", "Path"]);
            for r in &repos {
                let repo_name = std::path::Path::new(&r.repo_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                repo_table.add_row([&repo_name, &r.branch, &r.worktree_path]);
            }
            println!("{repo_table}");
        }

        let agents = agent::list(&db, ws.id)?;
        if agents.is_empty() {
            println!("  Agents: (none)");
        } else {
            let mut agent_table = Table::new();
            agent_table.set_header(["Agent", "Session", "Last Request"]);
            for a in &agents {
                agent_table.add_row([
                    Cell::new(&a.name),
                    Cell::new(a.session_id.as_deref().unwrap_or("—")),
                    Cell::new(a.last_request.as_deref().unwrap_or("—")),
                ]);
            }
            println!("{agent_table}");
        }
    }
    Ok(())
}
