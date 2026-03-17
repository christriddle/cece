use assert_cmd::Command;
use predicates::str::contains;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn init_cece(home: &std::path::Path) {
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home)
        .env("CECE_NON_INTERACTIVE", "1")
        .arg("init")
        .assert()
        .success();
}

fn create_test_repo(path: &std::path::Path) {
    std::fs::create_dir_all(path).unwrap();
    StdCommand::new("git")
        .args(["init", "-b", "main"])
        .current_dir(path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "--allow-empty", "-m", "init"])
        .current_dir(path)
        .output()
        .unwrap();
}

#[test]
fn test_ws_list_empty() {
    let home = TempDir::new().unwrap();
    init_cece(home.path());
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .args(["ws", "list"])
        .assert()
        .success()
        .stdout(contains("No workspaces"));
}

#[test]
fn test_ws_delete_nonexistent_errors() {
    let home = TempDir::new().unwrap();
    init_cece(home.path());
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .args(["ws", "delete", "nonexistent"])
        .assert()
        .failure();
}

#[test]
fn test_ws_create_and_delete_with_repo() {
    let home = TempDir::new().unwrap();
    init_cece(home.path());

    // Create a temporary git repo to use as the workspace repo.
    let repo_dir = TempDir::new().unwrap();
    let repo_path = repo_dir.path().join("myrepo");
    create_test_repo(&repo_path);

    // Create a workspace pointing at the repo with an explicit branch name.
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .env("CECE_NON_INTERACTIVE", "1")
        .args([
            "ws",
            "create",
            "testws",
            "--repos",
            &repo_path.to_string_lossy(),
            "--branch",
            "feature-test",
        ])
        .assert()
        .success()
        .stdout(contains("added"));

    // ws list should show the new workspace.
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .env("CECE_NON_INTERACTIVE", "1")
        .args(["ws", "list"])
        .assert()
        .success()
        .stdout(contains("testws"));

    // The worktree directory should exist under ~/.cece/workspaces/testws/myrepo.
    let worktree_path = home.path().join(".cece/workspaces/testws/myrepo");
    assert!(
        worktree_path.exists(),
        "worktree dir should exist at {}",
        worktree_path.display()
    );

    // CLAUDE.md should exist in the workspace dir and mention the repo name.
    let claude_md = home.path().join(".cece/workspaces/testws/CLAUDE.md");
    assert!(claude_md.exists(), "CLAUDE.md should be created");
    let content = std::fs::read_to_string(&claude_md).unwrap();
    assert!(
        content.contains("myrepo"),
        "CLAUDE.md should contain the repo name"
    );

    // Delete the workspace.
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .env("CECE_NON_INTERACTIVE", "1")
        .args(["ws", "delete", "testws"])
        .assert()
        .success();

    // After deletion, ws list should not show it.
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .env("CECE_NON_INTERACTIVE", "1")
        .args(["ws", "list"])
        .assert()
        .success()
        .stdout(contains("No workspaces"));

    // The worktree directory should be gone.
    assert!(
        !worktree_path.exists(),
        "worktree dir should be removed after delete"
    );
}

#[test]
fn test_ws_create_duplicate_errors() {
    let home = TempDir::new().unwrap();
    init_cece(home.path());

    let repo_dir = TempDir::new().unwrap();
    let repo_path = repo_dir.path().join("duperepo");
    create_test_repo(&repo_path);

    // Create the workspace once.
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .env("CECE_NON_INTERACTIVE", "1")
        .args([
            "ws",
            "create",
            "dupews",
            "--repos",
            &repo_path.to_string_lossy(),
            "--branch",
            "feature-dupe",
        ])
        .assert()
        .success();

    // Attempt to create the same workspace again — should fail.
    Command::cargo_bin("cece")
        .unwrap()
        .env("HOME", home.path())
        .env("CECE_NON_INTERACTIVE", "1")
        .args([
            "ws",
            "create",
            "dupews",
            "--repos",
            &repo_path.to_string_lossy(),
            "--branch",
            "feature-dupe2",
        ])
        .assert()
        .failure();
}
