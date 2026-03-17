use anyhow::Result;
use comfy_table::{Cell, Color};

use super::styled_table;
use crate::{db::agent, db::workspace, open_db};

pub fn handle_status() -> Result<()> {
    let db = open_db()?;
    let workspaces = workspace::list(&db)?;

    if workspaces.is_empty() {
        println!("No workspaces. Run `cece ws create <name>` to get started.");
        return Ok(());
    }

    for ws in &workspaces {
        println!("\n  {}", ws.name);

        let repos = workspace::get_repos(&db, ws.id)?;
        if repos.is_empty() {
            println!("  (no repos)");
        } else {
            let mut repo_table = styled_table(&["Repo", "Branch"]);
            for r in &repos {
                let repo_name = std::path::Path::new(&r.repo_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                repo_table.add_row([Cell::new(&repo_name).fg(Color::Cyan), Cell::new(&r.branch)]);
            }
            println!("{repo_table}");
        }

        let agents = agent::list(&db, ws.id)?;
        if !agents.is_empty() {
            let mut agent_table = styled_table(&["Agent", "Last Request", "Last Response"]);
            for a in &agents {
                agent_table.add_row([
                    Cell::new(&a.name).fg(Color::Green),
                    Cell::new(a.last_request.as_deref().unwrap_or("—")),
                    Cell::new(a.last_response.as_deref().unwrap_or("—")),
                ]);
            }
            println!("{agent_table}");
        }
    }
    Ok(())
}
