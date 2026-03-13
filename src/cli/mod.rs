use clap::{Parser, Subcommand};
use clap_complete::Shell;

pub mod agent;
pub mod init;
pub mod status;
pub mod template;
pub mod workspace;

#[derive(Parser)]
#[command(name = "cece", about = "Manage workspaces of Git repos and AI agents", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize cece in your home directory
    Init,
    /// Manage workspaces
    #[command(subcommand, name = "ws")]
    Workspace(workspace::WorkspaceCommands),
    /// Manage agents in the current workspace
    #[command(subcommand)]
    Agent(agent::AgentCommands),
    /// Manage workspace templates
    #[command(subcommand)]
    Template(template::TemplateCommands),
    /// Show status of all workspaces and agents
    Status,
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
}
