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

    let resp: Value =
        serde_json::from_str(line.trim()).context("invalid JSON response from cmux")?;
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

fn send_text(surface_id: &str, text: &str) -> Result<()> {
    send_request(
        "surface.send_text",
        json!({"surface_id": surface_id, "text": text}),
    )
    .context("surface.send_text failed")?;
    Ok(())
}

fn focus_surface(surface_id: &str) -> Result<()> {
    send_request("surface.focus", json!({"surface_id": surface_id}))
        .context("surface.focus failed")?;
    Ok(())
}

fn split(direction: &str) -> Result<String> {
    let resp = send_request("surface.split", json!({"direction": direction}))
        .context("surface.split failed")?;
    resp.get("result")
        .and_then(|r| r.get("surface_id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
        .context("surface.split returned no surface_id")
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

/// Set up the command-center surface in a new workspace.
/// Uses the surface cmux auto-creates when the workspace is selected,
/// navigates it to `dir`, and returns its ID for storage in the DB.
pub fn open_surface(cmux_workspace_id: &str, dir: &Path) -> Result<String> {
    select_workspace(cmux_workspace_id)?;

    let surfaces = list_surface_ids()?;
    let surface_id = surfaces
        .into_iter()
        .next()
        .context("no surfaces found in new workspace")?;

    send_text(&surface_id, &format!("cd {}\n", dir.display()))?;
    Ok(surface_id)
}

/// Open a new agent surface in the workspace.
///
/// Layout:
/// - First agent: split the command-center UP → agent appears above it.
/// - Subsequent agents: split the last agent surface RIGHT → agents tile horizontally.
///
/// Pass `resume: true` to run `claude --continue` instead of a fresh `claude`.
/// Returns the new surface ID for storage as the agent's session_id.
pub fn new_agent_tab(
    cmux_workspace_id: &str,
    command_center_surface_id: &str,
    agent_name: &str,
    working_dir: &Path,
    resume: bool,
) -> Result<String> {
    select_workspace(cmux_workspace_id)?;

    let all_surfaces = list_surface_ids()?;
    let agent_surfaces: Vec<_> = all_surfaces
        .into_iter()
        .filter(|id| id != command_center_surface_id)
        .collect();

    let (split_from, direction) = match agent_surfaces.last() {
        None => (command_center_surface_id.to_string(), "up"),
        Some(last) => (last.clone(), "right"),
    };

    focus_surface(&split_from)?;
    let new_surface_id = split(direction)?;
    let claude_cmd = if resume { "claude --continue" } else { "claude" };
    send_text(
        &new_surface_id,
        &format!("cd {} && {claude_cmd}\n", working_dir.display()),
    )?;

    let _ = agent_name; // stored by caller, not needed for the split
    Ok(new_surface_id)
}

/// Close a surface. Ignores errors (e.g. surface already gone).
pub fn close_surface(surface_id: &str) {
    let _ = send_request("surface.close", json!({"surface_id": surface_id}));
}

/// Focus an existing agent surface using its stored surface ID.
pub fn select_agent_tab(surface_id: &str) -> Result<()> {
    send_request("surface.focus", json!({"surface_id": surface_id}))
        .context("surface.focus failed")?;
    Ok(())
}
