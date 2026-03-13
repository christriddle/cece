use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn test_status_no_workspaces() {
    let home = TempDir::new().unwrap();
    Command::cargo_bin("cece").unwrap()
        .env("HOME", home.path()).env("CECE_NON_INTERACTIVE", "1")
        .arg("init").assert().success();
    Command::cargo_bin("cece").unwrap()
        .env("HOME", home.path())
        .arg("status")
        .assert()
        .success()
        .stdout(contains("No workspaces"));
}
