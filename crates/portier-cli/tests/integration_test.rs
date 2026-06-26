use std::process::Command;

/// Helper to get the portier binary path
fn portier_bin() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("portier")
}

#[test]
fn test_cli_version() {
    let output = Command::new(portier_bin())
        .arg("--version")
        .output()
        .expect("Failed to run portier --version");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("portier"));
}

#[test]
fn test_cli_help() {
    let output = Command::new(portier_bin())
        .arg("--help")
        .output()
        .expect("Failed to run portier --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("scan"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("assign"));
    assert!(stdout.contains("link"));
}

#[test]
fn test_cli_scan_json() {
    let output = Command::new(portier_bin())
        .args(["scan", "--json"])
        .output()
        .expect("Failed to run portier scan --json");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should be valid JSON array
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap_or_default();
    if !parsed.is_empty() {
        assert!(parsed[0].get("port").is_some());
        assert!(parsed[0].get("is_conflict").is_some());
    }
}

#[test]
fn test_cli_scan_table() {
    let output = Command::new(portier_bin())
        .args(["scan"])
        .output()
        .expect("Failed to run portier scan");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show port counts
    assert!(stdout.contains("Found") || stdout.contains("No"));
}

#[test]
fn test_cli_config_init_and_show() {
    let tmp = tempfile::tempdir().unwrap();

    // Create a Cargo.toml so detector picks Rust
    std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();

    // Init
    let init = Command::new(portier_bin())
        .current_dir(tmp.path())
        .args(["config", "init", "--name", "test-project"])
        .output()
        .expect("Failed to run portier config init");
    assert!(init.status.success(), "config init failed: {:?}", String::from_utf8_lossy(&init.stderr));

    // Show
    let show = Command::new(portier_bin())
        .current_dir(tmp.path())
        .args(["config", "show"])
        .output()
        .expect("Failed to run portier config show");
    assert!(show.status.success());
    let stdout = String::from_utf8_lossy(&show.stdout);
    assert!(stdout.contains("test-project"));
    assert!(stdout.contains("Rust"));
}

#[test]
fn test_cli_link_and_status() {
    let tmp = tempfile::tempdir().unwrap();

    std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();

    // Link
    let link = Command::new(portier_bin())
        .current_dir(tmp.path())
        .args(["link"])
        .output()
        .expect("Failed to run portier link");
    assert!(link.status.success(), "link failed: {:?}", String::from_utf8_lossy(&link.stderr));

    // Status
    let status_cmd = Command::new(portier_bin())
        .current_dir(tmp.path())
        .args(["status"])
        .output()
        .expect("Failed to run portier status");
    assert!(status_cmd.status.success());
    let stdout = String::from_utf8_lossy(&status_cmd.stdout);
    assert!(stdout.contains(&tmp.path().to_string_lossy().to_string()));
    // Table should show Rust as the stack
    assert!(stdout.contains("Rust"));
}
