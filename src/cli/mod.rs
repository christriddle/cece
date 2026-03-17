use clap::{Parser, Subcommand};
use clap_complete::Shell;
use comfy_table::presets::NOTHING;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};

pub mod agent;
pub mod hook;
pub mod init;
pub mod list;
pub mod status;
pub mod template;
pub mod workspace;

/// Create a styled table with no borders, dimmed headers, constrained to terminal width.
pub(crate) fn styled_table(headers: &[&str]) -> Table {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    table.set_content_arrangement(ContentArrangement::DynamicFullWidth);
    let width = terminal_size::terminal_size()
        .map(|(w, _)| w.0)
        .unwrap_or(120);
    table.set_width(width);
    table.set_header(headers.iter().map(|h| {
        Cell::new(h)
            .fg(Color::DarkGrey)
            .add_attribute(Attribute::Bold)
    }));
    table
}

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
    /// Open a worktree in IntelliJ IDEA
    Idea {
        /// Workspace name. Inferred from current directory if omitted.
        workspace: Option<String>,
    },
    /// Open a worktree in Zed
    Zed {
        /// Workspace name. Inferred from current directory if omitted.
        workspace: Option<String>,
    },
    /// Open a worktree in VS Code
    Code {
        /// Workspace name. Inferred from current directory if omitted.
        workspace: Option<String>,
    },
    /// Open a worktree in Cursor
    Cursor {
        /// Workspace name. Inferred from current directory if omitted.
        workspace: Option<String>,
    },
    /// Internal hooks called by Claude Code
    #[command(subcommand, hide = true)]
    Hook(hook::HookCommands),
    /// Output completion values for shell scripts
    #[command(subcommand, name = "_complete", hide = true)]
    Complete(CompleteCommands),
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand)]
pub enum CompleteCommands {
    /// Output workspace names, one per line
    #[command(name = "workspaces")]
    Workspaces,
    /// Output agent names for a workspace, one per line
    #[command(name = "agents")]
    Agents {
        /// Workspace name
        workspace: String,
    },
}
