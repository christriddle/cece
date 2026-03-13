use clap::Subcommand;
#[derive(Subcommand)]
pub enum AgentCommands {}
pub fn handle_agent(_cmd: AgentCommands) -> anyhow::Result<()> { Ok(()) }
