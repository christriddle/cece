use clap::Subcommand;
use std::io::Read;

const CLIP_LEN: usize = 200;

#[derive(Subcommand)]
pub enum HookCommands {
    /// [Internal] Called by the Claude Code SessionStart hook to track session IDs
    #[command(name = "session-start", hide = true)]
    SessionStart,
    /// [Internal] Called by the Claude Code UserPromptSubmit hook to record the last request
    #[command(name = "user-prompt-submit", hide = true)]
    UserPromptSubmit,
    /// [Internal] Called by the Claude Code Stop hook to record the last response
    #[command(name = "stop", hide = true)]
    Stop,
}

pub fn handle_hook(cmd: HookCommands) {
    match cmd {
        HookCommands::SessionStart => run(session_start),
        HookCommands::UserPromptSubmit => run(user_prompt_submit),
        HookCommands::Stop => run(stop),
    }
}

/// Run a hook handler, printing any error to stderr (hooks must always exit 0).
fn run(f: fn(&serde_json::Value, &crate::db::Database) -> anyhow::Result<()>) {
    if let Err(e) = try_run(f) {
        eprintln!("cece hook: {e:#}");
    }
}

fn try_run(
    f: fn(&serde_json::Value, &crate::db::Database) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let payload: serde_json::Value = serde_json::from_str(&input)?;

    let db_path = crate::db_path()?;
    if !db_path.exists() {
        return Ok(());
    }
    let db = crate::db::Database::open(db_path)?;

    f(&payload, &db)
}

fn session_start(payload: &serde_json::Value, db: &crate::db::Database) -> anyhow::Result<()> {
    let session_id = str_field(payload, "session_id")?;

    // Prefer the explicit agent ID set by `cece agent create/switch` over cwd lookup.
    // Without this, two agents sharing the same working directory would be indistinguishable.
    let agent = if let Ok(id_str) = std::env::var("CECE_AGENT_ID") {
        if let Ok(id) = id_str.parse::<i64>() {
            crate::db::agent::get_by_id(db, id)?
        } else {
            None
        }
    } else {
        // `source=resume`: session_id is already stored, look it up directly.
        // `source=startup` without CECE_AGENT_ID: fall back to cwd (best effort).
        let source = payload["source"].as_str().unwrap_or("");
        if source == "resume" {
            crate::db::agent::find_by_claude_session_id(db, session_id)?
        } else {
            let cwd = str_field(payload, "cwd")?;
            crate::db::agent::find_by_working_dir(db, cwd)?
        }
    };

    let Some(agent) = agent else {
        return Ok(());
    };
    if agent.claude_session_id.as_deref() != Some(session_id) {
        crate::db::agent::update_claude_session(db, agent.id, session_id)?;
    }
    crate::db::agent::set_waiting_for_input(db, agent.id, false)?;
    Ok(())
}

fn user_prompt_submit(payload: &serde_json::Value, db: &crate::db::Database) -> anyhow::Result<()> {
    let session_id = str_field(payload, "session_id")?;
    let prompt = str_field(payload, "prompt")?;

    let Some(agent) = crate::db::agent::find_by_claude_session_id(db, session_id)? else {
        return Ok(());
    };
    crate::db::agent::update_last_request(db, agent.id, &clip(prompt))?;
    crate::db::agent::set_waiting_for_input(db, agent.id, false)?;
    Ok(())
}

fn stop(payload: &serde_json::Value, db: &crate::db::Database) -> anyhow::Result<()> {
    let session_id = str_field(payload, "session_id")?;

    let Some(agent) = crate::db::agent::find_by_claude_session_id(db, session_id)? else {
        return Ok(());
    };

    // last_assistant_message may be null or absent in some stop scenarios (e.g. Ctrl+C).
    // Only mark as waiting when Claude actually finished responding (not on a hard kill).
    if let Some(message) = payload["last_assistant_message"].as_str() {
        crate::db::agent::update_last_response(db, agent.id, &clip(message))?;
        crate::db::agent::set_waiting_for_input(db, agent.id, true)?;
    }
    Ok(())
}

fn str_field<'a>(payload: &'a serde_json::Value, key: &str) -> anyhow::Result<&'a str> {
    payload[key]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing '{key}' in hook payload"))
}

fn clip(s: &str) -> String {
    let s = s.trim();
    if s.len() <= CLIP_LEN {
        s.to_string()
    } else {
        // Clip at a word boundary if possible
        let boundary = s[..CLIP_LEN]
            .rfind(|c: char| c.is_whitespace())
            .unwrap_or(CLIP_LEN);
        format!("{}…", s[..boundary].trim_end())
    }
}
