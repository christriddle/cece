use anyhow::Result;
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Encode a directory path into the format Claude Code uses for session directories.
/// Claude Code stores sessions under ~/.claude/projects/<encoded-path>/
fn encode_project_path(dir: &Path) -> String {
    dir.to_string_lossy()
        .replace('/', "-")
        .trim_start_matches('-')
        .to_string()
}

fn claude_session_dir(working_dir: &str) -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let encoded = encode_project_path(Path::new(working_dir));
    Some(home.join(".claude").join("projects").join(encoded))
}

/// Read recent session log entries from Claude Code's session storage.
pub fn read_session_logs(working_dir: &str) -> Result<Vec<String>> {
    let Some(session_dir) = claude_session_dir(working_dir) else {
        return Ok(vec![]);
    };
    if !session_dir.exists() {
        return Ok(vec![]);
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&session_dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("jsonl") {
            let content = std::fs::read_to_string(entry.path())?;
            for line in content.lines().rev().take(20) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
                        entries.push(msg.to_string());
                    }
                }
            }
        }
    }
    Ok(entries)
}

/// Poll until no Claude Code session file has been modified in the last 5 seconds.
/// This is a best-effort heuristic.
pub fn wait_until_idle(working_dir: &str) -> Result<()> {
    let Some(session_dir) = claude_session_dir(working_dir) else {
        anyhow::bail!("no Claude Code session directory found for {working_dir}");
    };

    loop {
        thread::sleep(Duration::from_secs(2));
        let most_recent = std::fs::read_dir(&session_dir)?
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
            .max();

        if let Some(modified) = most_recent {
            let age = modified.elapsed().unwrap_or(Duration::MAX);
            if age > Duration::from_secs(5) {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }
}
