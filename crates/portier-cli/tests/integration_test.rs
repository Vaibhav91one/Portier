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
    assert!(
        init.status.success(),
        "config init failed: {:?}",
        String::from_utf8_lossy(&init.stderr)
    );

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
    // Isolate the global registry into a throwaway HOME so the test never
    // touches the user's ~/.config/portier/registry.json.
    let home = tempfile::tempdir().unwrap();

    std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();

    // Link
    let link = Command::new(portier_bin())
        .current_dir(tmp.path())
        .env("HOME", home.path())
        .args(["link"])
        .output()
        .expect("Failed to run portier link");
    assert!(
        link.status.success(),
        "link failed: {:?}",
        String::from_utf8_lossy(&link.stderr)
    );

    // Status
    let status_cmd = Command::new(portier_bin())
        .current_dir(tmp.path())
        .env("HOME", home.path())
        .args(["status"])
        .output()
        .expect("Failed to run portier status");
    assert!(status_cmd.status.success());
    let stdout = String::from_utf8_lossy(&status_cmd.stdout);
    // Table should show Rust as the stack
    assert!(stdout.contains("Rust"));
}

/// `portier run` should auto-register the project it launches so it becomes a
/// tracked project, while the scanner keeps seeing every port for conflicts.
#[test]
fn test_run_auto_registers_project() {
    let home = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    std::fs::write(
        proj.path().join("package.json"),
        r#"{"scripts":{"dev":"next dev"}}"#,
    )
    .unwrap();
    let base = proj
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let out = Command::new(portier_bin())
        .current_dir(proj.path())
        .env("HOME", home.path())
        .args(["run", "--", "sh", "-c", "true"])
        .output()
        .expect("Failed to run portier run");
    assert!(
        out.status.success(),
        "run failed: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );

    let reg = std::fs::read_to_string(home.path().join(".config/portier/registry.json")).unwrap();
    assert!(reg.contains(&base), "project not registered: {reg}");
    assert!(reg.contains("\"dev\""), "dev service not registered: {reg}");
    // PID should be cleared after the child exits (serde skips None).
    assert!(
        !reg.contains("\"pid\""),
        "PID should be cleared after exit: {reg}"
    );
}

/// Spec §9 end-to-end: a project whose configured port is already taken should
/// have its config files rewritten to a free port by `portier start`.
#[test]
fn test_e2e_start_rewrites_config_on_conflict() {
    use std::net::TcpListener;

    // Hold a real port so the allocator is forced to reassign.
    let busy = TcpListener::bind("127.0.0.1:0").unwrap();
    let busy_port = busy.local_addr().unwrap().port();

    let home = tempfile::tempdir().unwrap();
    let proj = tempfile::tempdir().unwrap();
    std::fs::write(
        proj.path().join("package.json"),
        r#"{"scripts":{"dev":"next dev"}}"#,
    )
    .unwrap();
    std::fs::write(proj.path().join(".env"), format!("PORT={busy_port}\n")).unwrap();

    let out = Command::new(portier_bin())
        .current_dir(proj.path())
        .env("HOME", home.path())
        .args(["start", "--yes"])
        .output()
        .expect("Failed to run portier start");
    assert!(
        out.status.success(),
        "start failed: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );

    // The .env should no longer point at the busy port.
    let env_after = std::fs::read_to_string(proj.path().join(".env")).unwrap();
    assert!(
        !env_after.contains(&format!("PORT={busy_port}")),
        "expected .env to be rewritten off the busy port, got: {env_after}"
    );
    assert!(env_after.contains("PORT="), "PORT key should remain");
}
