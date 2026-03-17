# Cece Enhancements Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add remove-repo, ws info, template-based workspace creation, and integration tests for branch resolution and git operations.

**Architecture:** Five independent features that each touch their own slice of the codebase. Remove-repo mirrors the existing add-repo/delete patterns. Template usage wires existing DB layer into `ws create`. Info command reads existing DB data. Tests create temporary git repos to exercise real git operations.

**Tech Stack:** Rust, clap, dialoguer, rusqlite, git CLI, assert_cmd + tempfile (tests)

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/cli/workspace.rs` | New `RemoveRepo` and `Info` subcommands, `--template` flag on `Create` |
| `src/db/workspace.rs` | New `remove_repo()` DB function |
| `src/cli/template.rs` | No changes (already complete) |
| `src/db/template.rs` | Remove `#[allow(dead_code)]` annotations |
| `src/git/mod.rs` | Remove stale `#[allow(dead_code)]` on `detect_default_branch` |
| `CLAUDE.md` | Update CLI command reference |
| `tests/workspace_test.rs` | Integration tests for ws create/delete with real git repos |
| `tests/git_test.rs` | New: integration tests for git module functions |

---

### Task 1: `workspace::remove_repo()` DB function

**Files:**
- Modify: `src/db/workspace.rs:122-161`
- Test: `src/db/workspace.rs` (unit tests at bottom)

- [ ] **Step 1: Write failing test for remove_repo**

Add to the `#[cfg(test)]` block in `src/db/workspace.rs`:

```rust
#[test]
fn test_remove_repo() {
    let db = Database::open_in_memory().unwrap();
    let ws_id = create(&db, "ws").unwrap();
    add_repo(&db, ws_id, "/repos/frontend", "main", "/cece/ws/frontend", false).unwrap();
    add_repo(&db, ws_id, "/repos/backend", "main", "/cece/ws/backend", false).unwrap();

    let removed = remove_repo(&db, ws_id, "/repos/frontend").unwrap();
    assert_eq!(removed.repo_path, "/repos/frontend");
    assert_eq!(removed.worktree_path, "/cece/ws/frontend");

    let repos = get_repos(&db, ws_id).unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].repo_path, "/repos/backend");
}

#[test]
fn test_remove_repo_nonexistent_errors() {
    let db = Database::open_in_memory().unwrap();
    let ws_id = create(&db, "ws").unwrap();
    let result = remove_repo(&db, ws_id, "/repos/ghost");
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib db::workspace::tests::test_remove_repo`
Expected: FAIL — `remove_repo` not defined

- [ ] **Step 3: Implement `remove_repo` function**

Add to `src/db/workspace.rs` after `add_repo`:

```rust
/// Remove a repo from a workspace by repo_path. Returns the removed repo record.
pub fn remove_repo(db: &Database, workspace_id: i64, repo_path: &str) -> Result<WorkspaceRepo> {
    // Fetch the record first so we can return it (needed for worktree cleanup).
    let mut stmt = db.conn().prepare(
        "SELECT id, workspace_id, repo_path, branch, worktree_path, branch_new
         FROM workspace_repos WHERE workspace_id = ?1 AND repo_path = ?2",
    )?;
    let repo = stmt
        .query_row((workspace_id, repo_path), |r| {
            Ok(WorkspaceRepo {
                id: r.get(0)?,
                workspace_id: r.get(1)?,
                repo_path: r.get(2)?,
                branch: r.get(3)?,
                worktree_path: r.get(4)?,
                branch_new: r.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                CeceError::Git(format!("repo '{}' not found in workspace", repo_path))
            }
            other => CeceError::Database(other),
        })?;

    db.conn().execute(
        "DELETE FROM workspace_repos WHERE workspace_id = ?1 AND repo_path = ?2",
        (workspace_id, repo_path),
    )?;
    Ok(repo)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib db::workspace::tests`
Expected: All workspace DB tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/db/workspace.rs
git commit -m "feat: add workspace::remove_repo DB function"
```

---

### Task 2: `cece ws remove-repo` CLI command

**Files:**
- Modify: `src/cli/workspace.rs:9-56` (add subcommand variant + handler dispatch)
- Modify: `src/cli/workspace.rs` (add `remove_repo_cmd` function near `add_repo_cmd`)

- [ ] **Step 1: Add `RemoveRepo` variant to `WorkspaceCommands`**

In the `WorkspaceCommands` enum, add after `AddRepo`:

```rust
    /// Remove a repo from an existing workspace
    RemoveRepo {
        /// Workspace name. Inferred from current directory if omitted.
        #[arg(long)]
        workspace: Option<String>,
        /// Repo path to remove. If omitted, prompted interactively.
        repo: Option<String>,
    },
```

Add to `handle_ws` match:

```rust
        WorkspaceCommands::RemoveRepo { workspace, repo } => remove_repo_cmd(workspace, repo),
```

- [ ] **Step 2: Implement `remove_repo_cmd`**

Add after `add_repo_cmd` in `src/cli/workspace.rs`:

```rust
fn remove_repo_cmd(workspace_arg: Option<String>, repo_arg: Option<String>) -> Result<()> {
    let db = open_db()?;

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
    let repos = workspace::get_repos(&db, ws.id)?;

    if repos.is_empty() {
        anyhow::bail!("workspace '{}' has no repos", ws_name);
    }

    let repo_path = match repo_arg {
        Some(p) => p,
        None => {
            let repo_labels: Vec<String> = repos
                .iter()
                .map(|r| {
                    let name = std::path::Path::new(&r.repo_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    format!("{} ({})", name, r.branch)
                })
                .collect();
            let selection = FuzzySelect::new()
                .with_prompt("Select repo to remove")
                .items(&repo_labels)
                .interact()?;
            repos[selection].repo_path.clone()
        }
    };

    let removed = workspace::remove_repo(&db, ws.id, &repo_path)?;

    // Clean up git worktree.
    let repo_p = std::path::Path::new(&removed.repo_path);
    let wt_p = std::path::Path::new(&removed.worktree_path);
    git::worktree_remove(repo_p, wt_p)?;
    if removed.branch_new {
        git::delete_branch(repo_p, &removed.branch).unwrap_or_else(|e| {
            eprintln!("warning: could not delete branch '{}': {e}", removed.branch)
        });
    }

    let repo_name = std::path::Path::new(&removed.repo_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    println!("  removed {} from workspace '{}'", repo_name, ws_name);

    // Regenerate CLAUDE.md.
    let remaining = workspace::get_repos(&db, ws.id)?;
    if remaining.is_empty() {
        // If no repos left, remove the CLAUDE.md.
        let ws_dir = cece_dir()?.join("workspaces").join(&ws_name);
        let claude_md = ws_dir.join("CLAUDE.md");
        if claude_md.exists() {
            std::fs::remove_file(&claude_md)?;
        }
    } else {
        let ws_dir = cece_dir()?.join("workspaces").join(&ws_name);
        let all_repo_branches = repo_branches_from_db(&remaining);
        write_workspace_claude_md(&ws_dir, &ws_name, &all_repo_branches)?;
    }

    Ok(())
}
```

- [ ] **Step 3: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt`
Expected: No errors

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/cli/workspace.rs
git commit -m "feat: add cece ws remove-repo command"
```

---

### Task 3: `--template` flag on `cece ws create`

**Files:**
- Modify: `src/cli/workspace.rs:11-19` (add `--template` arg to `Create`)
- Modify: `src/cli/workspace.rs:65-171` (wire template into `create()`)
- Modify: `src/db/template.rs` (remove `#[allow(dead_code)]` annotations)

- [ ] **Step 1: Add `--template` arg to `Create` variant**

```rust
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
```

Update `handle_ws` match to pass `template`:

```rust
        WorkspaceCommands::Create {
            name,
            repos,
            branch,
            template,
        } => create(&name, repos, branch, template),
```

- [ ] **Step 2: Update `create()` signature and add template loading**

Change signature to:

```rust
fn create(name: &str, mut repo_paths: Vec<String>, branch_override: Option<String>, template_name: Option<String>) -> Result<()> {
```

At the start of `create()`, before the repo gathering block, add template loading:

```rust
    // If a template is specified, load repos and branch template from it.
    let template_branch = if let Some(ref tpl_name) = template_name {
        let tpl = crate::db::template::get_by_name(&db, tpl_name)?;
        if repo_paths.is_empty() {
            repo_paths = tpl.repo_paths;
        }
        Some(tpl.branch_template)
    } else {
        None
    };
```

Then when resolving branches, if `template_branch` is set and no `branch_override`, temporarily override the branch template config. The simplest approach: if we have a template branch pattern, set it in config temporarily. But that's messy.

Better approach: pass the template's branch_template into `prompt_new_branch` so it uses the template's pattern instead of the global config. Modify `prompt_new_branch` to accept an optional override:

```rust
fn prompt_new_branch(db: &crate::db::Database, template_override: Option<&str>) -> Result<String> {
    let template = match template_override {
        Some(t) => t.to_string(),
        None => config::get(db, "branch_template")?
            .unwrap_or_else(|| "{initials}-{ticket}-{desc}".to_string()),
    };
    // ... rest unchanged
```

Thread the `template_branch` through `resolve_branches_per_repo` → `resolve_branch_for_repo` → `pick_branch_interactive` → `prompt_new_branch` and `pick_branch_for_subsequent_repo` → `prompt_new_branch`. This requires adding an `Option<&str>` parameter for `branch_template_override` through the chain:

- `resolve_branches_per_repo(db, repo_paths, branch_override, branch_template_override)`
- `resolve_branch_for_repo(db, repo_path, all_repo_paths, branch_override, branch_template_override)`
- `pick_branch_interactive(db, repo_path, branch_template_override)`
- `pick_branch_for_subsequent_repo(db, repo_path, repo_name, first_branch, branch_template_override)`
- `prompt_new_branch(db, branch_template_override)`

This is a mechanical threading change — every function gains one extra `Option<&str>` param, and only `prompt_new_branch` changes behavior. All existing call sites pass `None` except the template-aware path.

- [ ] **Step 3: Remove `#[allow(dead_code)]` from `src/db/template.rs`**

Remove the four `#[allow(dead_code)]` annotations and their comments from `Template`, `create`, `get_by_name`, `list`, and `delete`.

- [ ] **Step 4: Remove stale `#[allow(dead_code)]` from `detect_default_branch` in `src/git/mod.rs`**

Remove `#[allow(dead_code)] // used in future task (Task 7)` from line 147. The function is now used.

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt`
Expected: No errors

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/cli/workspace.rs src/db/template.rs src/git/mod.rs
git commit -m "feat: add --template flag to cece ws create"
```

---

### Task 4: `cece ws info <name>` command

**Files:**
- Modify: `src/cli/workspace.rs:9-56` (add `Info` subcommand)
- Modify: `src/cli/workspace.rs` (add `info()` function)

- [ ] **Step 1: Add `Info` variant to `WorkspaceCommands`**

```rust
    /// Show details of a specific workspace
    Info {
        /// Workspace name. Inferred from current directory if omitted.
        name: Option<String>,
    },
```

Add to `handle_ws` match:

```rust
        WorkspaceCommands::Info { name } => info(name),
```

- [ ] **Step 2: Implement `info()` function**

Add after `list()`:

```rust
fn info(name_arg: Option<String>) -> Result<()> {
    let db = open_db()?;

    let ws_name = match name_arg {
        Some(name) => name,
        None => {
            let cwd = std::env::current_dir().context("cannot determine current directory")?;
            workspace::find_by_worktree(&db, &cwd)?
                .map(|ws| ws.name)
                .context("cannot infer workspace from current directory — pass a name")?
        }
    };
    let ws = workspace::get_by_name(&db, &ws_name)?;
    let repos = workspace::get_repos(&db, ws.id)?;
    let agents = crate::db::agent::list(&db, ws.id)?;
    let ws_dir = cece_dir()?.join("workspaces").join(&ws_name);

    println!("Workspace: {}", ws.name);
    println!("Created:   {}", ws.created_at);
    println!("Path:      {}", ws_dir.display());
    if let Some(cmux_id) = &ws.cmux_workspace_id {
        println!("Cmux ID:   {}", cmux_id);
    }

    if repos.is_empty() {
        println!("\nRepos: (none)");
    } else {
        println!();
        let mut table = comfy_table::Table::new();
        table.set_header(["Repo", "Branch", "New?", "Worktree"]);
        for r in &repos {
            let repo_name = std::path::Path::new(&r.repo_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            table.add_row([
                repo_name,
                r.branch.clone(),
                if r.branch_new { "yes".to_string() } else { "no".to_string() },
                r.worktree_path.clone(),
            ]);
        }
        println!("{table}");
    }

    if agents.is_empty() {
        println!("\nAgents: (none)");
    } else {
        println!();
        let mut table = comfy_table::Table::new();
        table.set_header(["Agent", "Session", "Last Request"]);
        for a in &agents {
            table.add_row([
                a.name.clone(),
                a.session_id.clone().unwrap_or_else(|| "—".to_string()),
                a.last_request.clone().unwrap_or_else(|| "—".to_string()),
            ]);
        }
        println!("{table}");
    }

    Ok(())
}
```

- [ ] **Step 3: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt`
Expected: No errors

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/cli/workspace.rs
git commit -m "feat: add cece ws info command"
```

---

### Task 5: Integration tests for git operations

**Files:**
- Create: `tests/git_test.rs`

These tests create real temporary git repos and exercise the functions in `src/git/mod.rs`.

- [ ] **Step 1: Create `tests/git_test.rs` with helper**

```rust
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Create a bare git repo and a working clone of it.
/// Returns (bare_dir, clone_dir) wrapped in TempDirs to manage lifetime.
fn setup_git_repo() -> (TempDir, TempDir) {
    let bare_dir = TempDir::new().unwrap();
    let clone_dir = TempDir::new().unwrap();

    // Init bare repo
    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(bare_dir.path())
        .output()
        .unwrap();

    // Clone it
    Command::new("git")
        .args([
            "clone",
            &bare_dir.path().to_string_lossy(),
            &clone_dir.path().to_string_lossy(),
        ])
        .output()
        .unwrap();

    // Create an initial commit so branches work
    let repo = clone_dir.path();
    Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "commit", "--allow-empty", "-m", "init"])
        .output()
        .unwrap();
    Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "push", "origin", "main"])
        .output()
        .unwrap();

    (bare_dir, clone_dir)
}
```

- [ ] **Step 2: Add `test_branch_exists`**

```rust
#[test]
fn test_branch_exists() {
    let (_bare, clone) = setup_git_repo();
    let repo = clone.path();

    // 'main' should exist as a local branch
    assert!(cece::git::branch_exists(repo, "main"));

    // nonexistent branch should not
    assert!(!cece::git::branch_exists(repo, "nonexistent"));
}
```

- [ ] **Step 3: Add `test_list_branches`**

```rust
#[test]
fn test_list_branches() {
    let (_bare, clone) = setup_git_repo();
    let repo = clone.path();

    // Create a second branch
    Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "branch", "feature-x"])
        .output()
        .unwrap();

    let branches = cece::git::list_branches(repo).unwrap();
    assert!(branches.contains(&"main".to_string()));
    assert!(branches.contains(&"feature-x".to_string()));
}
```

- [ ] **Step 4: Add `test_current_branch`**

```rust
#[test]
fn test_current_branch() {
    let (_bare, clone) = setup_git_repo();
    let repo = clone.path();

    let branch = cece::git::current_branch(repo).unwrap();
    assert_eq!(branch, "main");
}
```

- [ ] **Step 5: Add `test_detect_default_branch`**

```rust
#[test]
fn test_detect_default_branch() {
    let (_bare, clone) = setup_git_repo();
    let repo = clone.path();

    let default = cece::git::detect_default_branch(repo).unwrap();
    assert_eq!(default, "main");
}
```

- [ ] **Step 6: Add `test_worktree_add_and_remove`**

```rust
#[test]
fn test_worktree_add_and_remove() {
    let (_bare, clone) = setup_git_repo();
    let repo = clone.path();
    let wt_dir = TempDir::new().unwrap();
    let wt_path = wt_dir.path().join("my-worktree");

    // Add worktree with new branch
    let target = cece::git::BranchTarget::New {
        name: "test-branch".to_string(),
        start_point: None,
    };
    cece::git::worktree_add(repo, &wt_path, &target).unwrap();
    assert!(wt_path.exists());

    // The branch should now exist
    assert!(cece::git::branch_exists(repo, "test-branch"));

    // find_worktree_for_branch should find it
    let found = cece::git::find_worktree_for_branch(repo, "test-branch").unwrap();
    assert!(found.is_some());

    // Remove the worktree
    cece::git::worktree_remove(repo, &wt_path).unwrap();
    assert!(!wt_path.exists());
}
```

- [ ] **Step 7: Add `test_worktree_add_with_start_point`**

```rust
#[test]
fn test_worktree_add_with_start_point() {
    let (_bare, clone) = setup_git_repo();
    let repo = clone.path();
    let wt_dir = TempDir::new().unwrap();
    let wt_path = wt_dir.path().join("from-origin");

    let target = cece::git::BranchTarget::New {
        name: "from-origin-branch".to_string(),
        start_point: Some("origin/main".to_string()),
    };
    cece::git::worktree_add(repo, &wt_path, &target).unwrap();
    assert!(wt_path.exists());
    assert!(cece::git::branch_exists(repo, "from-origin-branch"));

    // Cleanup
    cece::git::worktree_remove(repo, &wt_path).unwrap();
}
```

- [ ] **Step 8: Add `test_expand_branch_template` (sanity check from integration level)**

```rust
#[test]
fn test_expand_branch_template_integration() {
    let mut vars = std::collections::HashMap::new();
    vars.insert("initials", "cr");
    vars.insert("ticket", "OPEN-456");
    vars.insert("desc", "add-tests");
    let result = cece::git::expand_branch_template("{initials}-{ticket}-{desc}", &vars);
    assert_eq!(result, "cr-OPEN-456-add-tests");
}
```

- [ ] **Step 9: Ensure `git` module is public in `src/lib.rs`**

Check that `src/lib.rs` exposes `pub mod git;`. If it doesn't, add it. The integration tests use `cece::git::*`.

- [ ] **Step 10: Run all tests**

Run: `cargo test`
Expected: All tests PASS (existing + new git tests)

- [ ] **Step 11: Commit**

```bash
git add tests/git_test.rs src/lib.rs
git commit -m "test: add integration tests for git operations"
```

---

### Task 6: Integration tests for workspace create/delete with real git repos

**Files:**
- Modify: `tests/workspace_test.rs`

These tests exercise `cece ws create` and `cece ws delete` end-to-end with real repos and `--branch` to skip interactive prompts.

- [ ] **Step 1: Add helper to create a test git repo**

Add to `tests/workspace_test.rs`:

```rust
use std::process::Command as StdCommand;

/// Create a git repo with an initial commit at the given path.
fn create_test_repo(path: &std::path::Path) {
    std::fs::create_dir_all(path).unwrap();
    StdCommand::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "--allow-empty", "-m", "init"])
        .current_dir(path)
        .output()
        .unwrap();
}
```

- [ ] **Step 2: Add `test_ws_create_and_delete_with_repo`**

```rust
#[test]
fn test_ws_create_and_delete_with_repo() {
    let home = TempDir::new().unwrap();
    init_cece(home.path());

    // Create a test git repo
    let repo_dir = home.path().join("repos").join("my-repo");
    create_test_repo(&repo_dir);

    // Create workspace with --branch to skip interactive prompts
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "ws", "create", "test-ws",
            "--repos", &repo_dir.to_string_lossy(),
            "--branch", "test-branch",
        ])
        .assert()
        .success()
        .stdout(contains("added my-repo"));

    // Verify workspace appears in list
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .args(["ws", "list"])
        .assert()
        .success()
        .stdout(contains("test-ws"));

    // Verify worktree directory was created
    let wt_path = home.path().join(".cece").join("workspaces").join("test-ws").join("my-repo");
    assert!(wt_path.exists());

    // Verify CLAUDE.md was generated
    let claude_md = home.path().join(".cece").join("workspaces").join("test-ws").join("CLAUDE.md");
    assert!(claude_md.exists());
    let content = std::fs::read_to_string(&claude_md).unwrap();
    assert!(content.contains("my-repo"));

    // Delete workspace
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .args(["ws", "delete", "test-ws"])
        .assert()
        .success();

    // Verify it's gone
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .args(["ws", "list"])
        .assert()
        .success()
        .stdout(contains("No workspaces"));

    // Verify worktree directory was removed
    assert!(!wt_path.exists());
}
```

- [ ] **Step 3: Add `test_ws_create_duplicate_errors`**

```rust
#[test]
fn test_ws_create_duplicate_errors() {
    let home = TempDir::new().unwrap();
    init_cece(home.path());

    let repo_dir = home.path().join("repos").join("my-repo");
    create_test_repo(&repo_dir);

    // Create first workspace
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "ws", "create", "dup-ws",
            "--repos", &repo_dir.to_string_lossy(),
            "--branch", "branch-1",
        ])
        .assert()
        .success();

    // Creating same-named workspace should fail
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "ws", "create", "dup-ws",
            "--repos", &repo_dir.to_string_lossy(),
            "--branch", "branch-2",
        ])
        .assert()
        .failure();
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add tests/workspace_test.rs
git commit -m "test: add integration tests for ws create/delete with real git repos"
```

---

### Task 7: Update CLAUDE.md and clean up

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update CLI command reference in CLAUDE.md**

Add these lines to the CLI Commands section:

```
cece ws remove-repo [--workspace X]   # Remove a repo from a workspace
cece ws info [name]                   # Show details of a workspace
```

And update the `ws create` line:

```
cece ws create <name> [--template T]  # Create workspace (interactive repo/branch selection)
```

- [ ] **Step 2: Run final full test suite**

Run: `cargo clippy -- -D warnings && cargo fmt --check && cargo test`
Expected: All pass, no warnings

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with new commands"
```
