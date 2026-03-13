use anyhow::Result;
use std::path::Path;

pub fn select_workspace(_name: &str) -> Result<()> { Ok(()) }
pub fn new_agent_tab(_workspace: &str, _agent: &str, _dir: &Path) -> Result<String> {
    Ok(String::new())
}
pub fn select_agent_tab(_workspace: &str, _agent: &str) -> Result<()> { Ok(()) }
