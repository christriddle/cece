use anyhow::Result;
use clap::Subcommand;
use comfy_table::{Cell, Table};
use dialoguer::{Input, MultiSelect};
use std::collections::HashMap;

use crate::{cece_dir, db::config, db::repo, db::workspace, git, open_db};

#[derive(Subcommand)]
pub enum WorkspaceCommands {
    /// Create a new workspace
    Create {
        name: String,
        /// Repos to include (paths on disk). If omitted, prompted interactively.
        #[arg(long, num_args = 1..)]
        repos: Vec<String>,
        /// Branch name override (skips template expansion)
        #[arg(long)]
        branch: Option<String>,
    },
    /// List all workspaces
    List,
    /// Delete a workspace and its worktrees
    Delete { name: String },
    /// Switch to a workspace (prints path, or uses Cmux if configured)
    Switch { name: String },
}

pub fn handle_ws(cmd: WorkspaceCommands) -> Result<()> {
    match cmd {
        WorkspaceCommands::Create { name, repos, branch } => create(&name, repos, branch),
        WorkspaceCommands::List => list(),
        WorkspaceCommands::Delete { name } => delete(&name),
        WorkspaceCommands::Switch { name } => switch(&name),
    }
}

fn create(name: &str, mut repo_paths: Vec<String>, branch_override: Option<String>) -> Result<()> {
    let db = open_db()?;

    // Gather repos interactively if not provided
    if repo_paths.is_empty() {
        let known = repo::list(&db)?;
        if known.is_empty() {
            let path: String = Input::new()
                .with_prompt("Enter a repo path")
                .interact_text()?;
            repo_paths.push(path);
        } else {
            let selections = MultiSelect::new()
                .with_prompt("Select repos to include (space to toggle, enter to confirm)")
                .items(&known)
                .interact()?;
            for i in selections {
                repo_paths.push(known[i].clone());
            }
            let add_more: String = Input::new()
                .with_prompt("Add another repo path (blank to skip)")
                .allow_empty(true)
                .interact_text()?;
            if !add_more.is_empty() {
                repo_paths.push(add_more);
            }
        }
    }

    if repo_paths.is_empty() {
        anyhow::bail!("no repos selected");
    }

    // Determine branch name
    let branch = match branch_override {
        Some(b) => b,
        None => {
            let template = config::get(&db, "branch_template")?
                .unwrap_or_else(|| "{initials}-{ticket}-{desc}".to_string());

            if template.contains('{') {
                let initials: String = Input::new().with_prompt("Your initials").interact_text()?;
                let ticket: String = Input::new().with_prompt("Ticket number").allow_empty(true).interact_text()?;
                let desc: String = Input::new().with_prompt("Short description").interact_text()?;
                let mut vars = HashMap::new();
                vars.insert("initials", initials.as_str());
                vars.insert("ticket", ticket.as_str());
                vars.insert("desc", desc.as_str());
                git::expand_branch_template(&template, &vars)
            } else {
                template
            }
        }
    };

    let ws_id = workspace::create(&db, name)?;
    let ws_dir = cece_dir()?.join("workspaces").join(name);

    for repo_path_str in &repo_paths {
        let repo_path = std::path::Path::new(repo_path_str);
        let repo_name = repo_path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());
        let worktree_path = ws_dir.join(&repo_name);

        git::worktree_add(repo_path, &worktree_path, &branch)?;
        workspace::add_repo(&db, ws_id, repo_path_str, &branch, &worktree_path.to_string_lossy())?;
        repo::add(&db, repo_path_str)?;

        println!("  added {} → {}", repo_name, worktree_path.display());
    }

    println!("Workspace '{}' created (branch: {})", name, branch);
    Ok(())
}

fn list() -> Result<()> {
    let db = open_db()?;
    let workspaces = workspace::list(&db)?;

    if workspaces.is_empty() {
        println!("No workspaces. Run `cece ws create <name>` to create one.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(["Name", "Repos", "Created"]);
    for ws in &workspaces {
        let repos = workspace::get_repos(&db, ws.id)?;
        let repo_names: Vec<_> = repos.iter()
            .map(|r| std::path::Path::new(&r.repo_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default())
            .collect();
        table.add_row([
            Cell::new(&ws.name),
            Cell::new(repo_names.join(", ")),
            Cell::new(&ws.created_at[..10]),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn delete(name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, name)?;
    let repos = workspace::get_repos(&db, ws.id)?;

    for r in &repos {
        let repo_path = std::path::Path::new(&r.repo_path);
        let worktree_path = std::path::Path::new(&r.worktree_path);
        if worktree_path.exists() {
            git::worktree_remove(repo_path, worktree_path)
                .unwrap_or_else(|e| eprintln!("warning: {e}"));
        }
    }

    let ws_dir = cece_dir()?.join("workspaces").join(name);
    if ws_dir.exists() {
        std::fs::remove_dir_all(&ws_dir).ok();
    }

    workspace::delete(&db, name)?;
    println!("Workspace '{}' deleted.", name);
    Ok(())
}

fn switch(name: &str) -> Result<()> {
    let db = open_db()?;
    workspace::get_by_name(&db, name)?; // validates it exists

    let ws_dir = cece_dir()?.join("workspaces").join(name);
    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");

    if cmux_enabled {
        crate::cmux::select_workspace(name)?;
        println!("Switched to workspace '{}' in Cmux.", name);
    } else {
        println!("{}", ws_dir.display());
        eprintln!("(Tip: use `cd $(cece ws switch {})` to change directories)", name);
    }
    Ok(())
}
