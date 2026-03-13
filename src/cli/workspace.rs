use clap::Subcommand;
#[derive(Subcommand)]
pub enum WorkspaceCommands {}
pub fn handle_ws(_cmd: WorkspaceCommands) -> anyhow::Result<()> { Ok(()) }
