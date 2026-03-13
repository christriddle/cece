use clap::Subcommand;
#[derive(Subcommand)]
pub enum TemplateCommands {}
pub fn handle_template(_cmd: TemplateCommands) -> anyhow::Result<()> { Ok(()) }
