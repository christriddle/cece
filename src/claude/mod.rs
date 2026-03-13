use anyhow::Result;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

/// Encode a directory path into the format Claude Code uses for session directories.
/// Claude Code stores sessions under ~/.claude/projects/<encoded-path>/
/// The encoding replaces every `/` with `-` (preserving the leading dash from the root slash).
fn encode_project_path(dir: &Path) -> String {
    dir.to_string_lossy().replace('/', "-")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_project_path_preserves_leading_dash() {
        // Claude Code encodes /Users/alice/dev/myrepo as -Users-alice-dev-myrepo
        let encoded = encode_project_path(Path::new("/Users/alice/dev/myrepo"));
        assert_eq!(encoded, "-Users-alice-dev-myrepo");
    }

    #[test]
    fn test_encode_project_path_matches_real_session_dirs() {
        // Verified against ~/.claude/projects/ on this machine
        let encoded = encode_project_path(Path::new("/Users/chris.riddle/dev/cece"));
        assert_eq!(encoded, "-Users-chris.riddle-dev-cece");
    }
}

const WATCH_TIMEOUT: Duration = Duration::from_secs(30 * 60); // 30 minutes
const WATCH_IDLE_THRESHOLD: Duration = Duration::from_secs(5);
const WATCH_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Poll until no Claude Code session file has been modified in the last 5 seconds.
/// Returns an error if no idle state is reached within 30 minutes.
/// This is a best-effort heuristic.
pub fn wait_until_idle(working_dir: &str) -> Result<()> {
    let Some(session_dir) = claude_session_dir(working_dir) else {
        anyhow::bail!("no Claude Code session directory found for {working_dir}");
    };

    let deadline = Instant::now() + WATCH_TIMEOUT;

    loop {
        thread::sleep(WATCH_POLL_INTERVAL);

        if Instant::now() >= deadline {
            anyhow::bail!(
                "agent watch timed out after 30 minutes — agent may still be running in {}",
                working_dir
            );
        }

        let most_recent = std::fs::read_dir(&session_dir)?
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
            .max();

        if let Some(modified) = most_recent {
            let age = modified.elapsed().unwrap_or(Duration::MAX);
            if age > WATCH_IDLE_THRESHOLD {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }
}
