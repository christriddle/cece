use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

fn cmux_available() -> bool {
    Command::new("cmux")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Switch the active Cmux workspace.
pub fn select_workspace(name: &str) -> Result<()> {
    if !cmux_available() {
        anyhow::bail!("cmux is not installed or not in PATH");
    }
    let status = Command::new("cmux")
        .args(["select-workspace", "--workspace", name])
        .status()
        .context("failed to run cmux")?;
    if !status.success() {
        anyhow::bail!("cmux select-workspace failed");
    }
    Ok(())
}

/// Open a new Cmux tab for an agent and return a synthetic session identifier.
pub fn new_agent_tab(workspace: &str, agent_name: &str, working_dir: &Path) -> Result<String> {
    if !cmux_available() {
        anyhow::bail!("cmux is not installed or not in PATH");
    }
    let status = Command::new("cmux")
        .args([
            "new-tab",
            "--workspace",
            workspace,
            "--name",
            agent_name,
            "--",
            "bash",
            "-c",
            &format!("cd {} && claude", working_dir.display()),
        ])
        .status()
        .context("failed to run cmux new-tab")?;

    if !status.success() {
        anyhow::bail!("cmux new-tab failed");
    }
    Ok(format!("cmux:{}:{}", workspace, agent_name))
}

/// Switch to an existing agent tab in Cmux.
pub fn select_agent_tab(workspace: &str, agent_name: &str) -> Result<()> {
    if !cmux_available() {
        anyhow::bail!("cmux is not installed or not in PATH");
    }
    let status = Command::new("cmux")
        .args(["select-tab", "--workspace", workspace, "--name", agent_name])
        .status()
        .context("failed to run cmux select-tab")?;
    if !status.success() {
        anyhow::bail!("cmux select-tab failed");
    }
    Ok(())
}
