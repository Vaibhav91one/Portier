use clap::Args;
use std::collections::HashSet;
use std::path::Path;

use libportier::registry::Registry;

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Command and arguments to run
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
    /// Preferred port (overrides project config and the PORT env var)
    #[arg(long)]
    pub port: Option<u16>,
    /// Project service whose port to allocate (and whose PID to track)
    #[arg(long)]
    pub service: Option<String>,
    /// Run the command unchanged — do not allocate or inject a port
    #[arg(long)]
    pub no_inject: bool,
}

fn is_dev_server(cmd: &str) -> bool {
    matches!(
        cmd,
        "npm" | "node" | "python" | "python3" | "cargo" | "next" | "vite" | "webpack" | "serve"
    )
}

/// Resolve a preferred port for this project: the named service's port from the
/// registry, else the first registered service, else a `PORT=` in the project's
/// `.env`. Returns `None` if nothing is configured.
fn project_preferred(root: &Path, service: &Option<String>) -> Option<u16> {
    let path_key = root.to_string_lossy().to_string();
    if let Ok(reg) = Registry::load() {
        if let Some(entry) = reg.get_project(&path_key) {
            if let Some(svc) = service {
                if let Some(s) = entry.services.get(svc) {
                    return Some(s.preferred);
                }
            }
            if let Some(s) = entry.services.values().next() {
                return Some(s.preferred);
            }
        }
    }
    libportier::config::extract_env_port(root)
        .ok()
        .and_then(|m| m.get("PORT").copied())
}

/// Record the chosen port + child PID on the project's service, if the project
/// is registered and a `--service` was named. Best-effort; ignores absence.
fn track_pid(root: &Path, service: &Option<String>, port: u16, pid: Option<u32>) {
    let (Some(svc), path_key) = (service, root.to_string_lossy().to_string()) else {
        return;
    };
    let _ = Registry::update(|reg| {
        if let Some(entry) = reg.get_project_mut(&path_key) {
            if let Some(s) = entry.services.get_mut(svc) {
                s.assigned = port;
                s.pid = pid;
            }
        }
        Ok(())
    });
}

pub fn run(args: RunArgs) -> anyhow::Result<()> {
    if args.command.is_empty() {
        println!("Usage: portier run [--port N] [--service NAME] -- <command>");
        return Ok(());
    }

    let cwd = std::env::current_dir()?;
    let project = libportier::resolve_project_root(&cwd);

    // --no-inject: preserve the old behavior — warn about conflicts, run as-is.
    if args.no_inject {
        let statuses = libportier::scanner::scan()?;
        let conflicts = statuses.iter().filter(|s| s.is_conflict).count();
        if conflicts > 0 {
            println!("Detected {conflicts} port conflict(s). Run `portier scan` for details.");
        }
        return spawn(&args.command, None, &project, &args.service);
    }

    // Resolve the preferred port: explicit flag > parent PORT env > project config > 3000.
    let parent_port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok());
    let preferred = args
        .port
        .or(parent_port)
        .or_else(|| project_preferred(&cwd, &args.service))
        .unwrap_or(3000);

    // Scan once (fast — native APIs) and pick a free port within the
    // configured range.
    let settings = libportier::Settings::load();
    let statuses = libportier::scanner::scan()?;
    let in_use: HashSet<u16> = statuses.iter().map(|s| s.port).collect();
    let port = libportier::pick_free_port(preferred, &in_use, Some(settings.port_range()));

    if port != preferred {
        println!("Port {preferred} is busy — using {port} instead.");
    } else {
        println!("Using port {port}.");
    }
    if is_dev_server(&args.command[0]) {
        println!("  Injected PORT={port} (use --no-inject to disable).");
    }

    // For arg-style servers (next -p, vite --port, django runserver), also
    // rewrite the flag so it can't override the injected PORT.
    let command = match libportier::inject::apply_port_to_args(&args.command, port) {
        Some(rewritten) => {
            println!("  Set the port flag on the command.");
            rewritten
        }
        None => args.command.clone(),
    };

    spawn(&command, Some(port), &project, &args.service)
}

/// Spawn the command, optionally injecting the allocated port, and wait.
fn spawn(
    command: &[String],
    port: Option<u16>,
    project: &Option<(String, std::path::PathBuf)>,
    service: &Option<String>,
) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new(&command[0]);
    cmd.args(&command[1..])
        .env("PORTIER_ACTIVE", "true")
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());

    if let Some(port) = port {
        for (key, value) in libportier::framework_env_vars(command, port) {
            cmd.env(key, value);
        }
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn command: {}", e))?;

    // Record the running PID on the project's service (best-effort).
    if let (Some(port), Some((_, root))) = (port, project) {
        track_pid(root, service, port, Some(child.id()));
    }

    let status = child.wait()?;

    // Clear the PID once the child exits so the registry doesn't keep a stale one.
    if let (Some(port), Some((_, root))) = (port, project) {
        track_pid(root, service, port, None);
    }

    if !status.success() {
        eprintln!("Command exited with: {:?}", status.code());
    }
    Ok(())
}
