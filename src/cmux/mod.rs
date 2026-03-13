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

fn find_workspace_id(name: &str) -> Result<String> {
    let resp = send_request("workspace.list", json!({}))?;
    let workspaces = resp
        .get("result")
        .and_then(|r| r.as_array())
        .context("unexpected workspace.list response")?;

    workspaces
        .iter()
        .find(|ws| ws.get("name").and_then(|n| n.as_str()) == Some(name))
        .and_then(|ws| ws.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
        .with_context(|| format!("no cmux workspace named '{name}'"))
}

/// Switch the active Cmux workspace by name.
pub fn select_workspace(name: &str) -> Result<()> {
    let id = find_workspace_id(name)?;
    send_request("workspace.select", json!({"workspace_id": id}))
        .context("workspace.select failed")?;
    Ok(())
}

/// Open a new split surface in the named workspace and start Claude Code in it.
pub fn new_agent_tab(workspace: &str, agent_name: &str, working_dir: &Path) -> Result<String> {
    let ws_id = find_workspace_id(workspace)?;
    send_request("workspace.select", json!({"workspace_id": ws_id}))?;

    let cmd = format!("cd {} && claude", working_dir.display());
    let resp = send_request(
        "surface.split",
        json!({"direction": "right", "command": cmd, "name": agent_name}),
    )
    .context("surface.split failed")?;

    let surface_id = resp
        .get("result")
        .and_then(|r| r.get("id"))
        .and_then(|id| id.as_str())
        .unwrap_or("")
        .to_string();

    Ok(if surface_id.is_empty() {
        format!("cmux:{}:{}", workspace, agent_name)
    } else {
        surface_id
    })
}

/// Focus an existing agent surface in Cmux.
pub fn select_agent_tab(workspace: &str, agent_name: &str) -> Result<()> {
    // Ensure we're on the right workspace first
    let ws_id = find_workspace_id(workspace)?;
    send_request("workspace.select", json!({"workspace_id": ws_id}))?;

    let resp = send_request("surface.list", json!({})).context("surface.list failed")?;
    let surfaces = resp
        .get("result")
        .and_then(|r| r.as_array())
        .context("unexpected surface.list response")?;

    let surface_id = surfaces
        .iter()
        .find(|s| s.get("name").and_then(|n| n.as_str()) == Some(agent_name))
        .and_then(|s| s.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
        .with_context(|| format!("no cmux surface named '{agent_name}'"))?;

    send_request("surface.focus", json!({"surface_id": surface_id}))
        .context("surface.focus failed")?;
    Ok(())
}
