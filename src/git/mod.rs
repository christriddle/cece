use crate::error::{CeceError, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

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

/// Create a git worktree at `worktree_path` on a new branch named `branch`.
pub fn worktree_add(repo_path: &Path, worktree_path: &Path, branch: &str) -> Result<()> {
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent).map_err(CeceError::Io)?;
    }
    let output = Command::new("git")
        .args([
            "-C",
            &repo_path.to_string_lossy(),
            "worktree",
            "add",
            "-b",
            branch,
            &worktree_path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| CeceError::Git(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CeceError::Git(format!("git worktree add failed: {stderr}")));
    }
    Ok(())
}

/// Remove a git worktree.
pub fn worktree_remove(repo_path: &Path, worktree_path: &Path) -> Result<()> {
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
