use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn init_cece(home: &std::path::Path) {
    Command::cargo_bin("cece").unwrap()
        .env("HOME", home).env("CECE_NON_INTERACTIVE", "1")
        .arg("init").assert().success();
}

#[test]
fn test_agent_list_no_workspace_errors() {
    let home = TempDir::new().unwrap();
    init_cece(home.path());
    Command::cargo_bin("cece").unwrap()
        .env("HOME", home.path())
        .args(["agent", "list", "--workspace", "nonexistent"])
        .assert()
        .failure()
        .stderr(contains("not found"));
}
