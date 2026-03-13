use assert_cmd::Command;
use tempfile::TempDir;

fn cece_cmd(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("cece").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn test_init_creates_db() {
    let home = TempDir::new().unwrap();
    cece_cmd(home.path())
        .arg("init")
        .env("CECE_NON_INTERACTIVE", "1")
        .assert()
        .success();
    assert!(home.path().join(".cece").join("cece.db").exists());
}

#[test]
fn test_init_twice_is_idempotent() {
    let home = TempDir::new().unwrap();
    for _ in 0..2 {
        cece_cmd(home.path())
            .arg("init")
            .env("CECE_NON_INTERACTIVE", "1")
            .assert()
            .success();
    }
}
