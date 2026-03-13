use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

fn socket_path() -> String {
    std::env::var("CMUX_SOCKET_PATH").unwrap_or_else(|_| "/tmp/cmux.sock".to_string())
}

fn send_request(method: &str, params: Value) -> Result<Value> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path)
        .with_context(|| format!("cannot connect to cmux socket at {path} — is cmux running?"))?;

    let request = json!({"id": "1", "method": method, "params": params});
    writeln!(stream, "{request}")?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line).context("no response from cmux")?;

    let resp: Value = serde_json::from_str(line.trim()).context("invalid JSON response from cmux")?;
    if resp.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        let code = resp
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_str())
            .unwrap_or("unknown");
        let msg = resp
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        anyhow::bail!("cmux error [{code}]: {msg}");
    }
    Ok(resp)
}

/// Create a new Cmux workspace with the given title. Returns the cmux workspace ID.
pub fn create_workspace(name: &str) -> Result<String> {
    let resp = send_request("workspace.create", json!({"title": name}))
        .context("workspace.create failed")?;
    resp.get("result")
        .and_then(|r| r.get("workspace_id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
        .context("workspace.create returned no workspace_id")
}

/// Switch the active Cmux workspace using its stored cmux workspace ID.
pub fn select_workspace(cmux_id: &str) -> Result<()> {
    send_request("workspace.select", json!({"workspace_id": cmux_id}))
        .context("workspace.select failed")?;
    Ok(())
}

/// Open a terminal surface in the given cmux workspace rooted at `dir`.
/// Replaces any auto-created surfaces that were added when the workspace was selected.
pub fn open_surface(cmux_workspace_id: &str, dir: &Path) -> Result<()> {
    select_workspace(cmux_workspace_id)?;

    // Snapshot whatever surfaces already exist (auto-created by cmux on workspace select).
    let existing = list_surface_ids()?;

    // Create a new surface, then send a cd command to navigate it to the worktree.
    let resp = send_request("surface.split", json!({"direction": "right"}))
        .context("surface.split failed")?;
    let surface_id = resp
        .get("result")
        .and_then(|r| r.get("surface_id"))
        .and_then(|id| id.as_str())
        .context("surface.split returned no surface_id")?;
    send_request(
        "surface.send_text",
        json!({"surface_id": surface_id, "text": format!("cd {}\n", dir.display())}),
    )
    .context("surface.send_text failed")?;

    // Close the pre-existing surfaces now that ours is open.
    for id in existing {
        let _ = send_request("surface.close", json!({"surface_id": id}));
    }

    Ok(())
}

fn list_surface_ids() -> Result<Vec<String>> {
    let resp = send_request("surface.list", json!({}))?;
    Ok(resp
        .get("result")
        .and_then(|r| r.get("surfaces"))
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default())
}

/// Open a new split surface in the given cmux workspace and start Claude Code in it.
/// Returns the cmux surface ID, which should be stored as the agent's session_id.
pub fn new_agent_tab(cmux_workspace_id: &str, agent_name: &str, working_dir: &Path) -> Result<String> {
    send_request("workspace.select", json!({"workspace_id": cmux_workspace_id}))?;

    let resp = send_request("surface.split", json!({"direction": "right"}))
        .context("surface.split failed")?;
    let surface_id = resp
        .get("result")
        .and_then(|r| r.get("surface_id"))
        .and_then(|id| id.as_str())
        .with_context(|| format!("surface.split for agent '{agent_name}' returned no surface_id"))?;
    send_request(
        "surface.send_text",
        json!({"surface_id": surface_id, "text": format!("cd {} && claude\n", working_dir.display())}),
    )
    .context("surface.send_text failed")?;

    Ok(surface_id.to_string())
}

/// Focus an existing agent surface using its stored surface ID.
pub fn select_agent_tab(surface_id: &str) -> Result<()> {
    send_request("surface.focus", json!({"surface_id": surface_id}))
        .context("surface.focus failed")?;
    Ok(())
}
