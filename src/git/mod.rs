use crate::error::{CeceError, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Whether to create a new branch or check out an existing one when adding a worktree.
pub enum BranchTarget {
    /// Create a new local branch with this name, optionally from a specific start point.
    New {
        name: String,
        start_point: Option<String>,
    },
    /// Check out an existing local or remote-tracking branch.
    Existing(String),
}

impl BranchTarget {
    pub fn name(&self) -> &str {
        match self {
            BranchTarget::New { name, .. } | BranchTarget::Existing(name) => name,
        }
    }

    pub fn is_new(&self) -> bool {
        matches!(self, BranchTarget::New { .. })
    }
}

/// Return true if the named branch exists in the repo (local or remote-tracking).
pub fn branch_exists(repo_path: &Path, branch: &str) -> bool {
    // Check local branch
    let local = Command::new("git")
        .args([
            "-C",
            &repo_path.to_string_lossy(),
            "rev-parse",
            "--verify",
            branch,
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if local {
        return true;
    }
    // Check remote-tracking branch (origin/<branch>)
    let remote = format!("origin/{branch}");
    Command::new("git")
        .args([
            "-C",
            &repo_path.to_string_lossy(),
            "rev-parse",
            "--verify",
            &remote,
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// List branches for the repo: local branches first, then remote-only branches
/// (stripped of the `origin/` prefix, excluding `HEAD`).
pub fn list_branches(repo_path: &Path) -> Result<Vec<String>> {
    let repo = repo_path.to_string_lossy();

    let local_out = Command::new("git")
        .args(["-C", &repo, "branch", "--format=%(refname:short)"])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;
    if !local_out.status.success() {
        let stderr = String::from_utf8_lossy(&local_out.stderr);
        return Err(CeceError::Git(format!("git branch failed: {stderr}")));
    }
    let local: Vec<String> = String::from_utf8_lossy(&local_out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let local_set: std::collections::HashSet<_> = local.iter().cloned().collect();

    let remote_out = Command::new("git")
        .args(["-C", &repo, "branch", "-r", "--format=%(refname:short)"])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;
    let remote_only: Vec<String> = if remote_out.status.success() {
        String::from_utf8_lossy(&remote_out.stdout)
            .lines()
            .filter_map(|l| l.trim().strip_prefix("origin/").map(|b| b.to_string()))
            .filter(|b| b != "HEAD" && !local_set.contains(b))
            .collect()
    } else {
        vec![]
    };

    let mut branches = local;
    branches.extend(remote_only);
    Ok(branches)
}

/// Return the worktree path that currently has `branch` checked out, if any.
/// Parses `git worktree list --porcelain` output.
pub fn find_worktree_for_branch(
    repo_path: &Path,
    branch: &str,
) -> Result<Option<std::path::PathBuf>> {
    let output = Command::new("git")
        .args([
            "-C",
            &repo_path.to_string_lossy(),
            "worktree",
            "list",
            "--porcelain",
        ])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;

    if !output.status.success() {
        return Ok(None);
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut current_path: Option<std::path::PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in text.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = Some(std::path::PathBuf::from(path));
            current_branch = None;
        } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
            current_branch = Some(b.to_string());
        } else if line.is_empty() {
            if current_branch.as_deref() == Some(branch) {
                return Ok(current_path);
            }
            current_path = None;
            current_branch = None;
        }
    }
    // Handle last entry (no trailing blank line)
    if current_branch.as_deref() == Some(branch) {
        return Ok(current_path);
    }
    Ok(None)
}

/// Detect the default branch of a repo (main or master).
#[allow(dead_code)] // used in future task (Task 7)
pub fn detect_default_branch(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args([
            "-C",
            &repo_path.to_string_lossy(),
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
        ])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_start_matches("refs/remotes/origin/")
            .to_string();
        return Ok(branch);
    }

    // Fallback: check for main then master
    for branch in ["main", "master"] {
        let check = Command::new("git")
            .args([
                "-C",
                &repo_path.to_string_lossy(),
                "rev-parse",
                "--verify",
                branch,
            ])
            .output()
            .map_err(|e| CeceError::Git(e.to_string()))?;
        if check.status.success() {
            return Ok(branch.to_string());
        }
    }

    Err(CeceError::Git(
        "cannot determine default branch".to_string(),
    ))
}

/// Fetch the latest refs from origin.
pub fn fetch_origin(repo_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["-C", &repo_path.to_string_lossy(), "fetch", "origin"])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CeceError::Git(format!("git fetch origin failed: {stderr}")));
    }
    Ok(())
}

/// Return the current branch name (e.g. "main", "feature-xyz").
pub fn current_branch(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args([
            "-C",
            &repo_path.to_string_lossy(),
            "rev-parse",
            "--abbrev-ref",
            "HEAD",
        ])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;
    if !output.status.success() {
        return Err(CeceError::Git(
            "could not determine current branch".to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Create a git worktree at `worktree_path` for the given branch target.
/// - `BranchTarget::New(name)` creates a new local branch.
/// - `BranchTarget::Existing(name)` checks out an existing local or remote-tracking branch.
pub fn worktree_add(repo_path: &Path, worktree_path: &Path, branch: &BranchTarget) -> Result<()> {
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent).map_err(CeceError::Io)?;
    }
    let repo = repo_path.to_string_lossy();
    let wt = worktree_path.to_string_lossy();

    let args: Vec<&str> = match branch {
        BranchTarget::New {
            name, start_point, ..
        } => {
            let mut a = vec!["-C", &repo, "worktree", "add", "-b", name, &wt];
            if let Some(sp) = start_point {
                a.push(sp);
            }
            a
        }
        BranchTarget::Existing(name) => vec!["-C", &repo, "worktree", "add", &wt, name],
    };

    let output = Command::new("git")
        .args(&args)
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CeceError::Git(format!("git worktree add failed: {stderr}")));
    }
    Ok(())
}

/// Remove a git worktree and prune any stale registrations.
/// Succeeds even if the worktree directory no longer exists.
pub fn worktree_remove(repo_path: &Path, worktree_path: &Path) -> Result<()> {
    // Attempt to remove the worktree. If the directory is already gone git may
    // report "not a working tree" — that's fine, we prune below regardless.
    if worktree_path.exists() {
        let output = Command::new("git")
            .args([
                "-C",
                &repo_path.to_string_lossy(),
                "worktree",
                "remove",
                "--force",
                &worktree_path.to_string_lossy(),
            ])
            .output()
            .map_err(|e| CeceError::Git(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CeceError::Git(format!(
                "git worktree remove failed: {stderr}"
            )));
        }
    }

    // Always prune to clean up any stale .git/worktrees/ entries.
    let prune = Command::new("git")
        .args(["-C", &repo_path.to_string_lossy(), "worktree", "prune"])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;

    if !prune.status.success() {
        let stderr = String::from_utf8_lossy(&prune.stderr);
        return Err(CeceError::Git(format!(
            "git worktree prune failed: {stderr}"
        )));
    }

    Ok(())
}

/// Delete a git branch. Returns Ok if the branch was deleted or didn't exist.
pub fn delete_branch(repo_path: &Path, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["-C", &repo_path.to_string_lossy(), "branch", "-D", branch])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;

    // Ignore "branch not found" — it may have already been deleted manually.
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("not found") {
            return Err(CeceError::Git(format!("git branch -D failed: {stderr}")));
        }
    }
    Ok(())
}

/// Expand a branch name template, replacing `{key}` placeholders with values from `vars`.
pub fn expand_branch_template(template: &str, vars: &HashMap<&str, &str>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_branch_template() {
        let mut vars = HashMap::new();
        vars.insert("initials", "cr");
        vars.insert("ticket", "OPEN-123");
        vars.insert("desc", "fix-auth");
        let result = expand_branch_template("{initials}-{ticket}-{desc}", &vars);
        assert_eq!(result, "cr-OPEN-123-fix-auth");
    }

    #[test]
    fn test_expand_partial_template() {
        let mut vars = HashMap::new();
        vars.insert("initials", "cr");
        let result = expand_branch_template("{initials}-feature", &vars);
        assert_eq!(result, "cr-feature");
    }

    #[test]
    fn test_expand_empty_vars_leaves_template_unchanged() {
        let vars = HashMap::new();
        let result = expand_branch_template("{initials}-{ticket}", &vars);
        assert_eq!(result, "{initials}-{ticket}");
    }
}
