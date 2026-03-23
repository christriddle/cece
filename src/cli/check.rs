use anyhow::Result;
use dialoguer::FuzzySelect;

use crate::{db::agent, open_db};

pub fn handle_check() -> Result<()> {
    let db = open_db()?;
    let waiting = agent::list_waiting(&db)?;

    if waiting.is_empty() {
        println!("No agents waiting for input.");
        return Ok(());
    }

    let labels: Vec<String> = waiting
        .iter()
        .map(|(a, ws_name)| {
            let snippet = a
                .last_request
                .as_deref()
                .unwrap_or("(no prompt recorded)");
            let snippet = if snippet.len() > 60 {
                &snippet[..60]
            } else {
                snippet
            };
            format!("{} / {}  —  {}", ws_name, a.name, snippet)
        })
        .collect();

    let selection = FuzzySelect::new()
        .with_prompt("Switch to agent")
        .items(&labels)
        .default(0)
        .interact()?;

    let (agent, ws_name) = &waiting[selection];
    crate::cli::agent::switch_to_agent(&db, agent, ws_name)
}
