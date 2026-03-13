use anyhow::Result;
use comfy_table::{Cell, Color, Table};

use crate::{db::agent, db::workspace, open_db};

pub fn handle_list() -> Result<()> {
    let db = open_db()?;
    let workspaces = workspace::list(&db)?;

    if workspaces.is_empty() {
        println!("No workspaces. Run `cece ws create <name>` to get started.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(["Workspace", "Agent", "Last Request", "Last Response"]);

    for ws in &workspaces {
        let agents = agent::list(&db, ws.id)?;
        if agents.is_empty() {
            table.add_row([
                Cell::new(&ws.name).fg(Color::Cyan),
                Cell::new("—"),
                Cell::new("—"),
                Cell::new("—"),
            ]);
        } else {
            for (i, a) in agents.iter().enumerate() {
                let ws_cell = if i == 0 {
                    Cell::new(&ws.name).fg(Color::Cyan)
                } else {
                    Cell::new("")
                };
                table.add_row([
                    ws_cell,
                    Cell::new(&a.name),
                    Cell::new(a.last_request.as_deref().unwrap_or("—")),
                    Cell::new(a.last_response.as_deref().unwrap_or("—")),
                ]);
            }
        }
    }

    println!("{table}");
    Ok(())
}
