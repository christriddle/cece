use crate::{cece_dir, db, db::config};
use anyhow::{Context, Result};
use dialoguer::{Confirm, Input};
use serde_json::json;
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

    register_claude_hook()?;

    if non_interactive {
        return Ok(());
    }

    // Branch template
    let existing_template = config::get(&db, "branch_template")?;
    let default_template = existing_template
        .as_deref()
        .unwrap_or("{initials}-{ticket}-{desc}");
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

/// Register the `cece hook session-start` hook in ~/.claude/settings.json.
/// Idempotent — skips if the hook is already present.
fn register_claude_hook() -> Result<()> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    let settings_path = home.join(".claude").join("settings.json");

    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or(json!({}))
    } else {
        json!({})
    };

    let cece_bin = std::env::current_exe().context("cannot determine cece binary path")?;
    let cece = cece_bin.display().to_string();

    register_event_hooks(
        &mut settings,
        "SessionStart",
        "hook session-start",
        &[
            json!({"matcher": "startup", "hooks": [async_cmd(&cece, "hook session-start")]}),
            json!({"matcher": "resume",  "hooks": [async_cmd(&cece, "hook session-start")]}),
            json!({"matcher": "clear",   "hooks": [async_cmd(&cece, "hook session-start")]}),
        ],
    )?;
    register_event_hooks(
        &mut settings,
        "UserPromptSubmit",
        "hook user-prompt-submit",
        &[json!({"matcher": "", "hooks": [async_cmd(&cece, "hook user-prompt-submit")]})],
    )?;
    register_event_hooks(
        &mut settings,
        "Stop",
        "hook stop",
        &[json!({"matcher": "", "hooks": [async_cmd(&cece, "hook stop")]})],
    )?;

    let content = serde_json::to_string_pretty(&settings)?;
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&settings_path, &content)?;
    println!("Registered Claude Code hooks in ~/.claude/settings.json");
    Ok(())
}

fn async_cmd(cece: &str, subcommand: &str) -> serde_json::Value {
    json!({"type": "command", "command": format!("{cece} {subcommand}"), "async": true})
}

/// Replace all cece entries for `event` that match `marker` with `entries`.
fn register_event_hooks(
    settings: &mut serde_json::Value,
    event: &str,
    marker: &str,
    entries: &[serde_json::Value],
) -> anyhow::Result<()> {
    let arr = settings
        .as_object_mut()
        .context("settings.json root must be an object")?
        .entry("hooks")
        .or_insert(json!({}))
        .as_object_mut()
        .context("hooks must be an object")?
        .entry(event)
        .or_insert(json!([]))
        .as_array_mut()
        .with_context(|| format!("{event} must be an array"))?;

    // Remove stale entries so re-running `cece init` always refreshes the binary path.
    arr.retain(|entry| {
        !entry
            .get("hooks")
            .and_then(|h| h.as_array())
            .is_some_and(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .is_some_and(|cmd| cmd.contains(marker))
                })
            })
    });

    arr.extend_from_slice(entries);
    Ok(())
}
