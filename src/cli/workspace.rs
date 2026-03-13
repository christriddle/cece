use anyhow::Result;
use clap::Subcommand;
use comfy_table::{Cell, Table};
use dialoguer::{Confirm, FuzzySelect, Input, MultiSelect};
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
        WorkspaceCommands::Create {
            name,
            repos,
            branch,
        } => create(&name, repos, branch),
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

    if workspace::get_by_name(&db, name).is_ok() {
        anyhow::bail!("a workspace named '{}' already exists", name);
    }

    // Determine branch target, retrying interactively on worktree conflicts.
    let first_repo = std::path::Path::new(&repo_paths[0]);
    let branch_target = resolve_branch(&db, first_repo, &repo_paths, branch_override)?;

    let ws_id = workspace::create(&db, name)?;
    let ws_dir = cece_dir()?.join("workspaces").join(name);

    for repo_path_str in &repo_paths {
        let repo_path = std::path::Path::new(repo_path_str);
        let repo_name = repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());
        let worktree_path = ws_dir.join(&repo_name);

        git::worktree_add(repo_path, &worktree_path, &branch_target)?;
        workspace::add_repo(
            &db,
            ws_id,
            repo_path_str,
            branch_target.name(),
            &worktree_path.to_string_lossy(),
            branch_target.is_new(),
        )?;
        repo::add(&db, repo_path_str)?;

        println!("  added {} → {}", repo_name, worktree_path.display());
    }

    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");
    if cmux_enabled {
        let cmux_id = crate::cmux::create_workspace(name)?;
        workspace::set_cmux_id(&db, ws_id, &cmux_id)?;
        // Open the command-center surface. With a single repo, land in the worktree
        // directly; with multiple repos, land in the workspace root so all repos
        // are visible as subdirectories.
        let start_dir = if repo_paths.len() == 1 {
            let repo_name = std::path::Path::new(&repo_paths[0])
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "repo".to_string());
            ws_dir.join(repo_name)
        } else {
            ws_dir.clone()
        };
        let surface_id = crate::cmux::open_surface(&cmux_id, &start_dir)?;
        workspace::set_cmux_surface_id(&db, ws_id, &surface_id)?;
        println!(
            "Workspace '{}' created (branch: {}) — Cmux workspace opened.",
            name,
            branch_target.name()
        );
    } else {
        println!(
            "Workspace '{}' created (branch: {}).",
            name,
            branch_target.name()
        );
    }
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
        let repo_names: Vec<_> = repos
            .iter()
            .map(|r| {
                std::path::Path::new(&r.repo_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            })
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

/// Determine the branch target, handling worktree conflicts with interactive retry.
/// If `branch_override` is given and conflicts, fails immediately (non-interactive).
fn resolve_branch(
    db: &crate::db::Database,
    first_repo: &std::path::Path,
    repo_paths: &[String],
    branch_override: Option<String>,
) -> Result<git::BranchTarget> {
    loop {
        let candidate = match &branch_override {
            Some(b) => {
                if git::branch_exists(first_repo, b) {
                    git::BranchTarget::Existing(b.clone())
                } else {
                    git::BranchTarget::New(b.clone())
                }
            }
            None => pick_branch_interactive(db, first_repo)?,
        };

        // Only existing branches can conflict — new branches don't exist yet.
        if let git::BranchTarget::New(_) = &candidate {
            return Ok(candidate);
        }

        // Check all repos for an existing worktree on this branch.
        let conflict = repo_paths.iter().find_map(|rp| {
            git::find_worktree_for_branch(std::path::Path::new(rp), candidate.name())
                .ok()
                .flatten()
        });

        match conflict {
            None => return Ok(candidate),
            Some(conflict_path) => {
                // See if it belongs to a cece workspace.
                let existing_ws = workspace::find_by_worktree(db, &conflict_path)?;
                match existing_ws {
                    Some(ws) => {
                        eprintln!(
                            "Branch '{}' is already used by workspace '{}'.",
                            candidate.name(),
                            ws.name
                        );
                        if branch_override.is_some() {
                            anyhow::bail!("cannot create workspace: branch already in use");
                        }
                        if Confirm::new()
                            .with_prompt(format!("Switch to workspace '{}' instead?", ws.name))
                            .default(true)
                            .interact()?
                        {
                            switch(&ws.name)?;
                            std::process::exit(0);
                        }
                        // Otherwise loop back to branch selection.
                    }
                    None => {
                        eprintln!(
                            "Branch '{}' is already checked out at '{}' (not a cece workspace).",
                            candidate.name(),
                            conflict_path.display()
                        );
                        if branch_override.is_some() {
                            anyhow::bail!("cannot create workspace: branch already checked out");
                        }
                        eprintln!("Please choose a different branch.");
                        // Loop back to branch selection.
                    }
                }
            }
        }
    }
}

/// Interactively pick a branch: FuzzySelect from existing branches plus a "new branch" option.
/// Defaults to "[ new branch ]". Main/master appears at the top of existing branches.
fn pick_branch_interactive(
    db: &crate::db::Database,
    repo_path: &std::path::Path,
) -> Result<git::BranchTarget> {
    let branches = git::list_branches(repo_path).unwrap_or_default();
    let default_branch =
        git::detect_default_branch(repo_path).unwrap_or_else(|_| "main".to_string());

    const NEW_BRANCH_ITEM: &str = "[ new branch ]";
    let mut items = vec![NEW_BRANCH_ITEM.to_string()];
    // Default branch (main/master) first, then the rest alphabetically.
    if branches.contains(&default_branch) {
        items.push(default_branch.clone());
    }
    items.extend(branches.iter().filter(|b| *b != &default_branch).cloned());

    let selection = FuzzySelect::new()
        .with_prompt("Branch")
        .items(&items)
        .default(0) // default to "[ new branch ]"
        .interact()?;

    if selection == 0 {
        Ok(git::BranchTarget::New(prompt_new_branch(db)?))
    } else {
        Ok(git::BranchTarget::Existing(items[selection].clone()))
    }
}

/// Prompt the user to name a new branch using the configured template.
fn prompt_new_branch(db: &crate::db::Database) -> Result<String> {
    let template = config::get(db, "branch_template")?
        .unwrap_or_else(|| "{initials}-{ticket}-{desc}".to_string());

    if !template.contains('{') {
        return Ok(template);
    }

    let saved_initials = config::get(db, "initials")?.unwrap_or_default();
    let mut initials_prompt = Input::new().with_prompt("Your initials");
    if !saved_initials.is_empty() {
        initials_prompt = initials_prompt.default(saved_initials.clone());
    }
    let initials: String = initials_prompt.interact_text()?;
    if initials != saved_initials {
        config::set(db, "initials", &initials)?;
    }

    let ticket: String = Input::new()
        .with_prompt("Ticket number")
        .allow_empty(true)
        .interact_text()?;
    let desc: String = Input::new()
        .with_prompt("Short description")
        .interact_text()?;

    let mut vars = HashMap::new();
    vars.insert("initials", initials.as_str());
    vars.insert("ticket", ticket.as_str());
    vars.insert("desc", desc.as_str());
    Ok(git::expand_branch_template(&template, &vars))
}

fn delete(name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, name)?;
    let repos = workspace::get_repos(&db, ws.id)?;

    for r in &repos {
        let repo_path = std::path::Path::new(&r.repo_path);
        let worktree_path = std::path::Path::new(&r.worktree_path);
        git::worktree_remove(repo_path, worktree_path)?;
        if r.branch_new {
            git::delete_branch(repo_path, &r.branch).unwrap_or_else(|e| {
                eprintln!("warning: could not delete branch '{}': {e}", r.branch)
            });
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

/// Return the cmux workspace ID for a cece workspace, creating a new cmux workspace
/// if one hasn't been set or the previously stored one no longer exists.
pub(crate) fn ensure_cmux_workspace(
    db: &crate::db::Database,
    ws: &workspace::Workspace,
    name: &str,
) -> Result<String> {
    if let Some(cmux_id) = ws.cmux_workspace_id.as_deref() {
        match crate::cmux::select_workspace(cmux_id) {
            Ok(()) => return Ok(cmux_id.to_string()),
            Err(e) if format!("{e:#}").contains("not_found") => {
                eprintln!("Cmux workspace no longer exists, creating a new one...");
            }
            Err(e) => return Err(e),
        }
    }
    let new_id = crate::cmux::create_workspace(name)?;
    workspace::set_cmux_id(db, ws.id, &new_id)?;
    Ok(new_id)
}

fn switch(name: &str) -> Result<()> {
    let db = open_db()?;
    let ws = workspace::get_by_name(&db, name)?;

    let ws_dir = cece_dir()?.join("workspaces").join(name);
    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");

    if cmux_enabled {
        let cmux_id = ensure_cmux_workspace(&db, &ws, name)?;
        crate::cmux::select_workspace(&cmux_id)?;
        println!("Switched to workspace '{}' in Cmux.", name);
    } else {
        println!("{}", ws_dir.display());
        eprintln!(
            "(Tip: use `cd $(cece ws switch {})` to change directories)",
            name
        );
    }
    Ok(())
}
