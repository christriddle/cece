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

    serde_json::from_str(line.trim()).context("invalid JSON response from cmux")
}

/// Find a cmux workspace ID by matching its title against `name`.
fn find_workspace_id(name: &str) -> Result<String> {
    let resp = send_request("workspace.list", json!({}))?;
    let workspaces = resp
        .get("result")
        .and_then(|r| r.get("workspaces"))
        .and_then(|w| w.as_array())
        .context("unexpected workspace.list response")?;

    workspaces
        .iter()
        .find(|ws| ws.get("title").and_then(|t| t.as_str()) == Some(name))
        .and_then(|ws| ws.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
        .with_context(|| format!("no cmux workspace with title '{name}'"))
}

/// Create a new Cmux workspace with the given title.
pub fn create_workspace(name: &str) -> Result<()> {
    send_request("workspace.create", json!({"title": name}))
        .context("workspace.create failed")?;
    Ok(())
}

/// Switch the active Cmux workspace by title.
pub fn select_workspace(name: &str) -> Result<()> {
    let id = find_workspace_id(name)?;
    send_request("workspace.select", json!({"workspace_id": id}))
        .context("workspace.select failed")?;
    Ok(())
}

/// Open a new split surface in the named workspace and start Claude Code in it.
/// Returns the cmux surface ID, which should be stored as the agent's session_id.
pub fn new_agent_tab(workspace: &str, agent_name: &str, working_dir: &Path) -> Result<String> {
    let ws_id = find_workspace_id(workspace)?;
    send_request("workspace.select", json!({"workspace_id": ws_id}))?;

    let cmd = format!("cd {} && claude", working_dir.display());
    let resp = send_request("surface.split", json!({"direction": "right", "command": cmd}))
        .context("surface.split failed")?;

    resp.get("result")
        .and_then(|r| r.get("surface_id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
        .with_context(|| format!("surface.split for agent '{agent_name}' returned no surface_id"))
}

/// Focus an existing agent surface using its stored surface ID.
pub fn select_agent_tab(surface_id: &str) -> Result<()> {
    send_request("surface.focus", json!({"surface_id": surface_id}))
        .context("surface.focus failed")?;
    Ok(())
}
