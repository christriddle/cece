use anyhow::{Context, Result};
use clap::Subcommand;
use comfy_table::{Cell, Table};
use dialoguer::{Confirm, FuzzySelect, Input, MultiSelect};
use std::collections::HashMap;

use crate::{cece_dir, db::agent, db::config, db::repo, db::template, db::workspace, git, open_db};

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
        /// Use a saved workspace template for repos and branch pattern
        #[arg(long)]
        template: Option<String>,
    },
    /// List all workspaces
    List,
    /// Delete a workspace and its worktrees
    Delete { name: String },
    /// Switch to a workspace (prints path, or uses Cmux if configured)
    Switch { name: String },
    /// Show details of a specific workspace
    Info {
        /// Workspace name. Inferred from current directory if omitted.
        name: Option<String>,
    },
    /// Add repos to an existing workspace
    AddRepo {
        /// Workspace name. Inferred from current directory if omitted.
        #[arg(long)]
        workspace: Option<String>,
        /// Repo paths to add. If omitted, prompted interactively.
        #[arg(num_args = 1..)]
        repos: Vec<String>,
        /// Branch name override (skips template expansion)
        #[arg(long)]
        branch: Option<String>,
    },
    /// Remove a repo from an existing workspace
    RemoveRepo {
        /// Workspace name. Inferred from current directory if omitted.
        #[arg(long)]
        workspace: Option<String>,
        /// Repo path to remove. If omitted, prompted interactively.
        repo: Option<String>,
    },
}

pub fn handle_ws(cmd: WorkspaceCommands) -> Result<()> {
    match cmd {
        WorkspaceCommands::Create {
            name,
            repos,
            branch,
            template,
        } => create(&name, repos, branch, template),
        WorkspaceCommands::List => list(),
        WorkspaceCommands::Delete { name } => delete(&name),
        WorkspaceCommands::Switch { name } => switch(&name),
        WorkspaceCommands::Info { name } => info(name),
        WorkspaceCommands::AddRepo {
            workspace,
            repos,
            branch,
        } => add_repo_cmd(workspace, repos, branch),
        WorkspaceCommands::RemoveRepo { workspace, repo } => remove_repo_cmd(workspace, repo),
    }
}

/// A repo path paired with its resolved branch target.
struct RepoBranch {
    path: String,
    branch: git::BranchTarget,
}

fn create(
    name: &str,
    mut repo_paths: Vec<String>,
    branch_override: Option<String>,
    template_name: Option<String>,
) -> Result<()> {
    let db = open_db()?;

    let template_branch = if let Some(ref tpl_name) = template_name {
        let tpl = template::get_by_name(&db, tpl_name)?;
        if repo_paths.is_empty() {
            repo_paths = tpl.repo_paths;
        }
        Some(tpl.branch_template)
    } else {
        None
    };

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
            loop {
                let add_more: String = Input::new()
                    .with_prompt("Add another repo path (blank to skip)")
                    .allow_empty(true)
                    .interact_text()?;
                if add_more.is_empty() {
                    break;
                }
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

    // Resolve branches per-repo.
    let repo_branches = resolve_branches_per_repo(
        &db,
        &repo_paths,
        branch_override,
        template_branch.as_deref(),
    )?;

    let ws_id = workspace::create(&db, name)?;
    let ws_dir = cece_dir()?.join("workspaces").join(name);

    let mut branch_names: Vec<String> = Vec::new();
    for rb in &repo_branches {
        let repo_path = std::path::Path::new(&rb.path);
        let repo_name = repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());
        let worktree_path = ws_dir.join(&repo_name);

        git::worktree_add(repo_path, &worktree_path, &rb.branch)?;
        workspace::add_repo(
            &db,
            ws_id,
            &rb.path,
            rb.branch.name(),
            &worktree_path.to_string_lossy(),
            rb.branch.is_new(),
        )?;
        repo::add(&db, &rb.path)?;

        println!(
            "  added {} ({}) → {}",
            repo_name,
            rb.branch.name(),
            worktree_path.display()
        );
        if !branch_names.contains(&rb.branch.name().to_string()) {
            branch_names.push(rb.branch.name().to_string());
        }
    }

    // Generate CLAUDE.md for the workspace.
    write_workspace_claude_md(&ws_dir, name, &repo_branches)?;

    let branch_summary = branch_names.join(", ");
    let cmux_enabled = config::get(&db, "cmux_enabled")?.as_deref() == Some("true");
    if cmux_enabled {
        let cmux_id = crate::cmux::create_workspace(name)?;
        workspace::set_cmux_id(&db, ws_id, &cmux_id)?;
        let start_dir = if repo_branches.len() == 1 {
            let repo_name = std::path::Path::new(&repo_branches[0].path)
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
            "Workspace '{}' created (branches: {}) — Cmux workspace opened.",
            name, branch_summary
        );
    } else {
        println!(
            "Workspace '{}' created (branches: {}).",
            name, branch_summary
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

fn info(name_arg: Option<String>) -> Result<()> {
    let db = open_db()?;

    let ws_name = match name_arg {
        Some(name) => name,
        None => {
            let cwd = std::env::current_dir().context("cannot determine current directory")?;
            workspace::find_by_worktree(&db, &cwd)?
                .map(|ws| ws.name)
                .context("cannot infer workspace from current directory — provide a name")?
        }
    };

    let ws = workspace::get_by_name(&db, &ws_name)?;
    let repos = workspace::get_repos(&db, ws.id)?;
    let agents = agent::list(&db, ws.id)?;

    let ws_dir = cece_dir()?.join("workspaces").join(&ws_name);

    println!("Workspace: {}", ws.name);
    println!("Created:   {}", &ws.created_at[..10]);
    println!("Path:      {}", ws_dir.display());
    if let Some(cmux_id) = &ws.cmux_workspace_id {
        println!("Cmux ID:   {}", cmux_id);
    }
    println!();

    // Repos table
    println!("Repos:");
    if repos.is_empty() {
        println!("  (none)");
    } else {
        let mut table = Table::new();
        table.set_header(["Repo", "Branch", "New?", "Worktree Path"]);
        for r in &repos {
            let repo_name = std::path::Path::new(&r.repo_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            table.add_row([
                Cell::new(&repo_name),
                Cell::new(&r.branch),
                Cell::new(if r.branch_new { "yes" } else { "no" }),
                Cell::new(&r.worktree_path),
            ]);
        }
        println!("{table}");
    }

    // Agents table
    println!("Agents:");
    if agents.is_empty() {
        println!("  (none)");
    } else {
        let mut table = Table::new();
        table.set_header(["Agent", "Session", "Last Request"]);
        for a in &agents {
            table.add_row([
                Cell::new(&a.name),
                Cell::new(a.claude_session_id.as_deref().unwrap_or("—")),
                Cell::new(
                    a.last_request
                        .as_deref()
                        .map(|s| if s.len() > 60 { &s[..60] } else { s })
                        .unwrap_or("—"),
                ),
            ]);
        }
        println!("{table}");
    }

    Ok(())
}

/// Resolve branch targets for each repo. With `--branch`, all repos share it.
/// Interactively, the first repo gets a full branch picker; for subsequent repos
/// "same branch" is offered as the default.
fn resolve_branches_per_repo(
    db: &crate::db::Database,
    repo_paths: &[String],
    branch_override: Option<String>,
    branch_template_override: Option<&str>,
) -> Result<Vec<RepoBranch>> {
    if let Some(ref b) = branch_override {
        // Non-interactive: all repos share the overridden branch.
        let first_repo = std::path::Path::new(&repo_paths[0]);
        let target = resolve_branch_for_repo(
            db,
            first_repo,
            repo_paths,
            &branch_override,
            branch_template_override,
        )?;
        return Ok(repo_paths
            .iter()
            .map(|p| RepoBranch {
                path: p.clone(),
                branch: if git::branch_exists(std::path::Path::new(p), b) {
                    git::BranchTarget::Existing(b.clone())
                } else {
                    git::BranchTarget::New {
                        name: b.clone(),
                        start_point: target
                            .as_ref()
                            .and_then(|t| match t {
                                git::BranchTarget::New { start_point, .. } => start_point.clone(),
                                _ => None,
                            })
                            .clone(),
                    }
                },
            })
            .collect());
    }

    let mut result: Vec<RepoBranch> = Vec::new();

    for (i, repo_path_str) in repo_paths.iter().enumerate() {
        let repo_path = std::path::Path::new(repo_path_str);
        let repo_name = repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());

        if i == 0 {
            // First repo: full branch picker.
            let branch = resolve_branch_for_repo(
                db,
                repo_path,
                repo_paths,
                &None,
                branch_template_override,
            )?
            .expect("branch selection should not be empty");
            result.push(RepoBranch {
                path: repo_path_str.clone(),
                branch,
            });
        } else {
            // Subsequent repos: offer "same as first" plus full picker.
            let first = &result[0].branch;
            let branch = pick_branch_for_subsequent_repo(
                db,
                repo_path,
                &repo_name,
                first,
                branch_template_override,
            )?;
            result.push(RepoBranch {
                path: repo_path_str.clone(),
                branch,
            });
        }
    }

    Ok(result)
}

/// Resolve a single branch target with worktree conflict retry.
fn resolve_branch_for_repo(
    db: &crate::db::Database,
    repo_path: &std::path::Path,
    all_repo_paths: &[String],
    branch_override: &Option<String>,
    branch_template_override: Option<&str>,
) -> Result<Option<git::BranchTarget>> {
    loop {
        let candidate = match branch_override {
            Some(b) => {
                if git::branch_exists(repo_path, b) {
                    git::BranchTarget::Existing(b.clone())
                } else {
                    git::BranchTarget::New {
                        name: b.clone(),
                        start_point: None,
                    }
                }
            }
            None => pick_branch_interactive(db, repo_path, branch_template_override)?,
        };

        if let git::BranchTarget::New { .. } = &candidate {
            return Ok(Some(candidate));
        }

        // Check for worktree conflict on this branch.
        let conflict = all_repo_paths.iter().find_map(|rp| {
            git::find_worktree_for_branch(std::path::Path::new(rp), candidate.name())
                .ok()
                .flatten()
        });

        match conflict {
            None => return Ok(Some(candidate)),
            Some(conflict_path) => {
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
                    }
                }
            }
        }
    }
}

/// For the 2nd+ repo, offer "same branch as first" as default, or pick a different one.
fn pick_branch_for_subsequent_repo(
    db: &crate::db::Database,
    repo_path: &std::path::Path,
    repo_name: &str,
    first_branch: &git::BranchTarget,
    branch_template_override: Option<&str>,
) -> Result<git::BranchTarget> {
    let same_label = format!("[ same branch - {} ]", first_branch.name());

    let branches = git::list_branches(repo_path).unwrap_or_default();
    let default_branch =
        git::detect_default_branch(repo_path).unwrap_or_else(|_| "main".to_string());
    let current_branch = git::current_branch(repo_path).unwrap_or_else(|_| "unknown".to_string());

    let new_from_main = format!("[ new branch - from {} ]", default_branch);
    let new_from_current = format!("[ new branch - from current ({}) ]", current_branch);

    let mut items = vec![
        same_label.clone(),
        new_from_main.clone(),
        new_from_current.clone(),
    ];
    if branches.contains(&default_branch) {
        items.push(default_branch.clone());
    }
    items.extend(branches.iter().filter(|b| *b != &default_branch).cloned());

    let selection = FuzzySelect::new()
        .with_prompt(format!("Branch for {}", repo_name))
        .items(&items)
        .default(0)
        .interact()?;

    if items[selection] == same_label {
        // Replicate the first repo's branch target for this repo.
        // If the first branch was new with a start point, resolve the start point
        // for *this* repo's default branch (it may differ from the first repo's).
        Ok(match first_branch {
            git::BranchTarget::New {
                name, start_point, ..
            } => {
                let resolved_start_point = if start_point.is_some() {
                    // The first repo used "from main" — do the same for this repo,
                    // using this repo's own default branch.
                    eprintln!("Fetching latest {} from origin...", default_branch);
                    git::fetch_origin(repo_path)?;
                    Some(format!("origin/{}", default_branch))
                } else {
                    None
                };
                git::BranchTarget::New {
                    name: name.clone(),
                    start_point: resolved_start_point,
                }
            }
            git::BranchTarget::Existing(name) => git::BranchTarget::Existing(name.clone()),
        })
    } else if items[selection] == new_from_main {
        eprintln!("Fetching latest {} from origin...", default_branch);
        git::fetch_origin(repo_path)?;
        let name = prompt_new_branch(db, branch_template_override)?;
        Ok(git::BranchTarget::New {
            name,
            start_point: Some(format!("origin/{}", default_branch)),
        })
    } else if items[selection] == new_from_current {
        let name = prompt_new_branch(db, branch_template_override)?;
        Ok(git::BranchTarget::New {
            name,
            start_point: None,
        })
    } else {
        Ok(git::BranchTarget::Existing(items[selection].clone()))
    }
}

/// Interactively pick a branch: FuzzySelect from existing branches plus two "new branch" options.
/// "New branch - from main/master" (default) fetches origin first.
/// "New branch - from current" branches from the current HEAD.
fn pick_branch_interactive(
    db: &crate::db::Database,
    repo_path: &std::path::Path,
    branch_template_override: Option<&str>,
) -> Result<git::BranchTarget> {
    let branches = git::list_branches(repo_path).unwrap_or_default();
    let default_branch =
        git::detect_default_branch(repo_path).unwrap_or_else(|_| "main".to_string());
    let current_branch = git::current_branch(repo_path).unwrap_or_else(|_| "unknown".to_string());

    let new_from_main = format!("[ new branch - from {} ]", default_branch);
    let new_from_current = format!("[ new branch - from current ({}) ]", current_branch);

    let mut items = vec![new_from_main.clone(), new_from_current.clone()];
    // Existing branches: default branch first, then the rest alphabetically.
    if branches.contains(&default_branch) {
        items.push(default_branch.clone());
    }
    items.extend(branches.iter().filter(|b| *b != &default_branch).cloned());

    let selection = FuzzySelect::new()
        .with_prompt("Branch")
        .items(&items)
        .default(0)
        .interact()?;

    if items[selection] == new_from_main {
        eprintln!("Fetching latest {} from origin...", default_branch);
        git::fetch_origin(repo_path)?;
        let name = prompt_new_branch(db, branch_template_override)?;
        Ok(git::BranchTarget::New {
            name,
            start_point: Some(format!("origin/{}", default_branch)),
        })
    } else if items[selection] == new_from_current {
        let name = prompt_new_branch(db, branch_template_override)?;
        Ok(git::BranchTarget::New {
            name,
            start_point: None,
        })
    } else {
        Ok(git::BranchTarget::Existing(items[selection].clone()))
    }
}

/// Prompt the user to name a new branch using the configured template.
fn prompt_new_branch(db: &crate::db::Database, template_override: Option<&str>) -> Result<String> {
    let template = match template_override {
        Some(t) => t.to_string(),
        None => config::get(db, "branch_template")?
            .unwrap_or_else(|| "{initials}-{ticket}-{desc}".to_string()),
    };

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

/// Write a CLAUDE.md into the workspace directory so that Claude Code agents
/// working in this workspace understand the repo layout and how worktrees work.
fn write_workspace_claude_md(
    ws_dir: &std::path::Path,
    ws_name: &str,
    repos: &[RepoBranch],
) -> Result<()> {
    std::fs::create_dir_all(ws_dir)?;

    let mut repo_table = String::new();
    for rb in repos {
        let repo_path = std::path::Path::new(&rb.path);
        let repo_name = repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());
        repo_table.push_str(&format!(
            "| {repo_name} | `{branch}` | `{dir}` | `{origin}` |\n",
            branch = rb.branch.name(),
            dir = repo_name,
            origin = rb.path,
        ));
    }

    let is_multi = repos.len() > 1;
    let multi_repo_note = if is_multi {
        "This is a **multi-repo workspace**. Each repo is in its own subdirectory. \
         When making cross-repo changes, coordinate commits across repos to keep them in sync.\n"
    } else {
        ""
    };

    let content = format!(
        r#"# Workspace: {ws_name}

{multi_repo_note}
## Repos

| Repo | Branch | Directory | Origin |
|------|--------|-----------|--------|
{repo_table}
## How This Workspace Works

Each repo above is a **git worktree** — a lightweight checkout that shares history with the
original clone listed in the Origin column. This means:

- **Do not run `git clone`** inside this workspace. The repos are already checked out.
- **Commits, branches, and stashes** are shared with the origin repo. A branch created here
  is visible from the origin, and vice versa.
- **`git pull`/`git push`** work normally from within any worktree directory.
- To see all worktrees for a repo: `git worktree list` (run from any worktree or the origin).

## Working In This Workspace

- `cd` into a repo's directory before running repo-specific commands (build, test, lint).
- Each repo may have its own CLAUDE.md with repo-specific instructions — read it if present.
- Keep changes focused on the branch for this workspace. Avoid switching branches inside a
  worktree; create a new workspace instead.

## Plans

If you create implementation plans, design docs, or other working documents, put them in the
`plans/` directory at the workspace root — **not inside any repo**. This keeps planning
artifacts out of version control and avoids polluting repo diffs.

```
{ws_name}/
  plans/          ← your plans go here
  {repo_dirs}```
"#,
        repo_dirs = repos
            .iter()
            .map(|rb| {
                {
                    let repo_path = std::path::Path::new(&rb.path);
                    let repo_name = repo_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "repo".to_string());
                    format!("  {repo_name}/")
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );

    let claude_md_path = ws_dir.join("CLAUDE.md");
    std::fs::write(&claude_md_path, content)?;

    std::fs::create_dir_all(ws_dir.join("plans"))?;

    Ok(())
}

/// Build RepoBranch entries from existing workspace_repos records (for CLAUDE.md regeneration).
fn repo_branches_from_db(repos: &[workspace::WorkspaceRepo]) -> Vec<RepoBranch> {
    repos
        .iter()
        .map(|r| RepoBranch {
            path: r.repo_path.clone(),
            branch: if r.branch_new {
                git::BranchTarget::New {
                    name: r.branch.clone(),
                    start_point: None,
                }
            } else {
                git::BranchTarget::Existing(r.branch.clone())
            },
        })
        .collect()
}

fn add_repo_cmd(
    workspace_arg: Option<String>,
    mut repo_paths: Vec<String>,
    branch_override: Option<String>,
) -> Result<()> {
    let db = open_db()?;

    // Resolve workspace from argument or cwd.
    let ws_name = match workspace_arg {
        Some(name) => name,
        None => {
            let cwd = std::env::current_dir().context("cannot determine current directory")?;
            workspace::find_by_worktree(&db, &cwd)?
                .map(|ws| ws.name)
                .context("cannot infer workspace from current directory — use --workspace")?
        }
    };
    let ws = workspace::get_by_name(&db, &ws_name)?;
    let ws_dir = cece_dir()?.join("workspaces").join(&ws_name);
    let existing_repos = workspace::get_repos(&db, ws.id)?;
    let existing_paths: Vec<String> = existing_repos.iter().map(|r| r.repo_path.clone()).collect();

    // Gather repos interactively if not provided.
    if repo_paths.is_empty() {
        let known = repo::list(&db)?;
        // Filter out repos already in this workspace.
        let available: Vec<String> = known
            .into_iter()
            .filter(|p| !existing_paths.contains(p))
            .collect();

        if available.is_empty() {
            let path: String = Input::new()
                .with_prompt("Enter a repo path")
                .interact_text()?;
            repo_paths.push(path);
        } else {
            let selections = MultiSelect::new()
                .with_prompt("Select repos to add (space to toggle, enter to confirm)")
                .items(&available)
                .interact()?;
            for i in selections {
                repo_paths.push(available[i].clone());
            }
        }
        loop {
            let add_more: String = Input::new()
                .with_prompt("Add another repo path (blank to skip)")
                .allow_empty(true)
                .interact_text()?;
            if add_more.is_empty() {
                break;
            }
            repo_paths.push(add_more);
        }
    }

    // Filter out repos already in the workspace.
    repo_paths.retain(|p| {
        if existing_paths.contains(p) {
            eprintln!("  skipping {} — already in workspace", p);
            false
        } else {
            true
        }
    });

    if repo_paths.is_empty() {
        anyhow::bail!("no new repos to add");
    }

    // Resolve branches per-repo.
    let repo_branches = resolve_branches_per_repo(&db, &repo_paths, branch_override, None)?;

    for rb in &repo_branches {
        let repo_path = std::path::Path::new(&rb.path);
        let repo_name = repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());
        let worktree_path = ws_dir.join(&repo_name);

        git::worktree_add(repo_path, &worktree_path, &rb.branch)?;
        workspace::add_repo(
            &db,
            ws.id,
            &rb.path,
            rb.branch.name(),
            &worktree_path.to_string_lossy(),
            rb.branch.is_new(),
        )?;
        repo::add(&db, &rb.path)?;

        println!(
            "  added {} ({}) → {}",
            repo_name,
            rb.branch.name(),
            worktree_path.display()
        );
    }

    // Regenerate CLAUDE.md with the full repo list.
    let all_repos = workspace::get_repos(&db, ws.id)?;
    let all_repo_branches = repo_branches_from_db(&all_repos);
    write_workspace_claude_md(&ws_dir, &ws_name, &all_repo_branches)?;

    println!(
        "Added {} repo(s) to workspace '{}'.",
        repo_branches.len(),
        ws_name
    );
    Ok(())
}

fn remove_repo_cmd(workspace_arg: Option<String>, repo_arg: Option<String>) -> Result<()> {
    let db = open_db()?;

    // Resolve workspace from argument or cwd.
    let ws_name = match workspace_arg {
        Some(name) => name,
        None => {
            let cwd = std::env::current_dir().context("cannot determine current directory")?;
            workspace::find_by_worktree(&db, &cwd)?
                .map(|ws| ws.name)
                .context("cannot infer workspace from current directory — use --workspace")?
        }
    };
    let ws = workspace::get_by_name(&db, &ws_name)?;
    let ws_dir = cece_dir()?.join("workspaces").join(&ws_name);
    let existing_repos = workspace::get_repos(&db, ws.id)?;

    if existing_repos.is_empty() {
        anyhow::bail!("workspace '{}' has no repos", ws_name);
    }

    // Resolve which repo to remove.
    let repo_path = match repo_arg {
        Some(path) => path,
        None => {
            let items: Vec<String> = existing_repos
                .iter()
                .map(|r| {
                    let repo_name = std::path::Path::new(&r.repo_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| r.repo_path.clone());
                    format!("{} ({})", repo_name, r.branch)
                })
                .collect();
            let selection = FuzzySelect::new()
                .with_prompt("Select repo to remove")
                .items(&items)
                .default(0)
                .interact()?;
            existing_repos[selection].repo_path.clone()
        }
    };

    let removed = workspace::remove_repo(&db, ws.id, &repo_path)?;

    // Remove the git worktree.
    let repo_path_obj = std::path::Path::new(&removed.repo_path);
    let worktree_path = std::path::Path::new(&removed.worktree_path);
    git::worktree_remove(repo_path_obj, worktree_path)?;

    // Delete branch if it was freshly created for this workspace.
    if removed.branch_new {
        git::delete_branch(repo_path_obj, &removed.branch).unwrap_or_else(|e| {
            eprintln!("warning: could not delete branch '{}': {e}", removed.branch)
        });
    }

    // Regenerate CLAUDE.md with remaining repos, or remove it if none left.
    let remaining_repos = workspace::get_repos(&db, ws.id)?;
    if remaining_repos.is_empty() {
        let claude_md_path = ws_dir.join("CLAUDE.md");
        if claude_md_path.exists() {
            std::fs::remove_file(&claude_md_path)?;
        }
    } else {
        let repo_branches = repo_branches_from_db(&remaining_repos);
        write_workspace_claude_md(&ws_dir, &ws_name, &repo_branches)?;
    }

    let repo_name = std::path::Path::new(&removed.repo_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| removed.repo_path.clone());
    println!("Removed repo '{}' from workspace '{}'.", repo_name, ws_name);
    Ok(())
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
