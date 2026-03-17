use clap::{Parser, Subcommand};
use clap_complete::Shell;

pub mod agent;
pub mod hook;
pub mod init;
pub mod list;
pub mod status;
pub mod template;
pub mod workspace;

#[derive(Parser)]
#[command(
    name = "cece",
    about = "Manage workspaces of Git repos and AI agents",
    version
)]
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
    /// List all workspaces and their agents
    List,
    /// Show status of all workspaces and agents
    Status,
    /// Open the current worktree in IntelliJ IDEA
    Idea,
    /// Open the current worktree in Zed
    Zed,
    /// Open the current worktree in VS Code
    Code,
    /// Open the current worktree in Cursor
    Cursor,
    /// Internal hooks called by Claude Code
    #[command(subcommand, hide = true)]
    Hook(hook::HookCommands),
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
}
