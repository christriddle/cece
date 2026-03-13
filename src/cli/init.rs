use crate::{cece_dir, db, db::config};
use anyhow::{Context, Result};
use dialoguer::{Confirm, Input};
use std::fs;

pub fn handle_init() -> Result<()> {
    let dir = cece_dir()?;
    let non_interactive = std::env::var("CECE_NON_INTERACTIVE").is_ok();

    if dir.exists() {
        if !non_interactive {
            println!("cece is already initialized at {}", dir.display());
        }
    } else {
        fs::create_dir_all(&dir).context("failed to create ~/.cece")?;
        println!("Initialized cece at {}", dir.display());
    }

    let db_path = dir.join("cece.db");
    let db = db::Database::open(&db_path)?;

    if non_interactive {
        return Ok(());
    }

    // Branch template
    let existing_template = config::get(&db, "branch_template")?;
    let default_template = existing_template.as_deref().unwrap_or("{initials}-{ticket}-{desc}");
    let branch_template: String = Input::new()
        .with_prompt("Branch name template")
        .with_initial_text(default_template)
        .interact_text()?;
    config::set(&db, "branch_template", &branch_template)?;

    // Cmux
    let use_cmux: bool = Confirm::new()
        .with_prompt("Enable Cmux integration?")
        .default(config::get(&db, "cmux_enabled")?.as_deref() == Some("true"))
        .interact()?;
    config::set(&db, "cmux_enabled", if use_cmux { "true" } else { "false" })?;

    println!("Configuration saved.");
    Ok(())
}
