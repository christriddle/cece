use cece::git::{
    branch_exists, current_branch, detect_default_branch, expand_branch_template,
    find_worktree_for_branch, list_branches, worktree_add, worktree_remove, BranchTarget,
};
use std::collections::HashMap;
use std::process::Command;
use tempfile::TempDir;

/// Create a bare git repo and a working clone.
fn setup_git_repo() -> (TempDir, TempDir) {
    let bare_dir = TempDir::new().unwrap();
    let clone_dir = TempDir::new().unwrap();

    // Init bare repo with explicit main branch
    Command::new("git")
        .args(["init", "--bare", "-b", "main"])
        .current_dir(bare_dir.path())
        .output()
        .unwrap();

    // Clone it
    Command::new("git")
        .args([
            "clone",
            &bare_dir.path().to_string_lossy().to_string(),
            &clone_dir.path().to_string_lossy().to_string(),
        ])
        .output()
        .unwrap();

    // Create initial commit and push
    let repo = clone_dir.path();
    let repo_str = repo.to_string_lossy().to_string();
    Command::new("git")
        .args(["-C", &repo_str, "commit", "--allow-empty", "-m", "init"])
        .output()
        .unwrap();
    Command::new("git")
        .args(["-C", &repo_str, "push", "origin", "main"])
        .output()
        .unwrap();

    (bare_dir, clone_dir)
}

#[test]
fn test_branch_exists() {
    let (_bare, clone) = setup_git_repo();
    let repo = clone.path();

    assert!(branch_exists(repo, "main"), "main branch should exist");
    assert!(
        !branch_exists(repo, "nonexistent"),
        "nonexistent branch should not exist"
    );
}

#[test]
fn test_list_branches() {
    let (_bare, clone) = setup_git_repo();
    let repo = clone.path();
    let repo_str = repo.to_string_lossy().to_string();

    // Create a feature branch
    Command::new("git")
        .args(["-C", &repo_str, "branch", "feature-x"])
        .output()
        .unwrap();

    let branches = list_branches(repo).unwrap();
    assert!(
        branches.contains(&"main".to_string()),
        "main should be in branches: {:?}",
        branches
    );
    assert!(
        branches.contains(&"feature-x".to_string()),
        "feature-x should be in branches: {:?}",
        branches
    );
}

#[test]
fn test_current_branch() {
    let (_bare, clone) = setup_git_repo();
    let branch = current_branch(clone.path()).unwrap();
    assert_eq!(branch, "main");
}

#[test]
fn test_detect_default_branch() {
    let (_bare, clone) = setup_git_repo();
    let default = detect_default_branch(clone.path()).unwrap();
    assert_eq!(default, "main");
}

#[test]
fn test_worktree_add_and_remove() {
    let (_bare, clone) = setup_git_repo();
    let repo = clone.path();

    let wt_dir = TempDir::new().unwrap();
    let wt_path = wt_dir.path().join("my-worktree");

    let branch = BranchTarget::New {
        name: "feature-wt".to_string(),
        start_point: None,
    };
    worktree_add(repo, &wt_path, &branch).unwrap();

    // Worktree path should exist after add
    assert!(wt_path.exists(), "worktree path should exist after add");

    // find_worktree_for_branch should locate it.
    // Canonicalize both paths to handle macOS /var -> /private/var symlinks.
    let found = find_worktree_for_branch(repo, "feature-wt").unwrap();
    assert!(found.is_some(), "should find worktree for feature-wt");
    assert_eq!(
        found.unwrap().canonicalize().unwrap(),
        wt_path.canonicalize().unwrap()
    );

    // Remove the worktree
    worktree_remove(repo, &wt_path).unwrap();

    // Path should no longer exist
    assert!(
        !wt_path.exists(),
        "worktree path should be gone after remove"
    );
}

#[test]
fn test_worktree_add_with_start_point() {
    let (_bare, clone) = setup_git_repo();
    let repo = clone.path();

    let wt_dir = TempDir::new().unwrap();
    let wt_path = wt_dir.path().join("feature-from-origin");

    let branch = BranchTarget::New {
        name: "feature-from-origin-main".to_string(),
        start_point: Some("origin/main".to_string()),
    };
    worktree_add(repo, &wt_path, &branch).unwrap();

    assert!(
        wt_path.exists(),
        "worktree path should exist when created from origin/main"
    );

    worktree_remove(repo, &wt_path).unwrap();
}

#[test]
fn test_expand_branch_template_integration() {
    let mut vars = HashMap::new();
    vars.insert("initials", "ab");
    vars.insert("ticket", "PROJ-42");
    vars.insert("desc", "my-feature");

    let result = expand_branch_template("{initials}-{ticket}-{desc}", &vars);
    assert_eq!(result, "ab-PROJ-42-my-feature");
}
