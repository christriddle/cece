use assert_cmd::Command;
use predicates::str::contains;
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
