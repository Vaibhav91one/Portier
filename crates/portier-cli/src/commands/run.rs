use clap::Args;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use libportier::registry::{Registry, ServiceEntry};

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Command and arguments to run
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
    /// Preferred port (overrides project config and the PORT env var)
    #[arg(long)]
    pub port: Option<u16>,
    /// Service name to register under (default: "dev")
    #[arg(long)]
    pub service: Option<String>,
    /// Run the command unchanged — do not allocate, inject, or track
    #[arg(long)]
    pub no_inject: bool,
}

fn is_dev_server(cmd: &str) -> bool {
    matches!(
        cmd,
        "npm"
            | "pnpm"
            | "yarn"
            | "bun"
            | "node"
            | "deno"
            | "python"
            | "python3"
            | "uvicorn"
            | "flask"
            | "rails"
            | "cargo"
            | "next"
            | "vite"
            | "webpack"
            | "serve"
    )
}

/// Everything needed to register and PID-track the running project.
struct Registration {
    root: PathBuf,
    name: String,
    stack: String,
    service: String,
    preferred: u16,
    assigned: u16,
}

/// Resolve a preferred port: the named service's port from the registry, else
/// the first registered service, else a `PORT=` in the project's `.env`.
fn project_preferred(root: &Path, service: &str) -> Option<u16> {
    let path_key = root.to_string_lossy().to_string();
    if let Ok(reg) = Registry::load() {
        if let Some(entry) = reg.get_project(&path_key) {
            if let Some(s) = entry.services.get(service) {
                return Some(s.preferred);
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

/// Register the project (adding it if new) and record the running PID.
///
/// Auto-added projects are marked `linked: false`; the port assignment is kept
/// across runs (sticky) — only the PID is cleared when the process exits.
fn track(r: &Registration, pid: Option<u32>) {
    let path_key = r.root.to_string_lossy().to_string();
    let _ = Registry::update(|reg| {
        let entry = reg.ensure_project(&path_key, &r.name, &r.stack);
        let svc = entry
            .services
            .entry(r.service.clone())
            .or_insert(ServiceEntry {
                preferred: r.preferred,
                assigned: r.assigned,
                pid,
            });
        svc.assigned = r.assigned;
        svc.pid = pid;
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

    // --no-inject: pure passthrough — warn about conflicts, run as-is, no tracking.
    if args.no_inject {
        let statuses = libportier::scanner::scan()?;
        let conflicts = statuses.iter().filter(|s| s.is_conflict).count();
        if conflicts > 0 {
            println!("Detected {conflicts} port conflict(s). Run `portier scan` for details.");
        }
        return spawn(&args.command, None, None);
    }

    let service_name = args.service.clone().unwrap_or_else(|| "dev".to_string());

    // Preferred port: explicit flag > parent PORT env > project config > 3000.
    let parent_port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok());
    let preferred = args
        .port
        .or(parent_port)
        .or_else(|| project_preferred(&cwd, &service_name))
        .unwrap_or(3000);

    // Scan once (native APIs) and pick a free port within the configured range.
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

    // For arg-style servers (next -p, vite --port, django runserver), rewrite
    // the flag too so it can't override the injected PORT.
    let command = match libportier::inject::apply_port_to_args(&args.command, port) {
        Some(rewritten) => {
            println!("  Set the port flag on the command.");
            rewritten
        }
        None => args.command.clone(),
    };

    // Register the project the moment it starts, if we're inside a real project
    // root. The scanner still sees every other port, so conflicts stay visible.
    let registration = project.as_ref().map(|(name, root)| {
        let stack = libportier::detector::detect_stack(root)
            .map(|d| d.stack.to_string())
            .unwrap_or_else(|_| "Unknown".to_string());
        Registration {
            root: root.clone(),
            name: name.clone(),
            stack,
            service: service_name.clone(),
            preferred,
            assigned: port,
        }
    });
    if let Some(r) = &registration {
        println!(
            "  Tracking \"{}\" → {} on {port} (portier status).",
            r.name, r.service
        );
    }

    spawn(&command, Some(port), registration)
}

/// Spawn the command, optionally injecting the port + tracking the project, and wait.
fn spawn(
    command: &[String],
    inject_port: Option<u16>,
    registration: Option<Registration>,
) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new(&command[0]);
    cmd.args(&command[1..])
        .env("PORTIER_ACTIVE", "true")
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());

    if let Some(port) = inject_port {
        for (key, value) in libportier::framework_env_vars(command, port) {
            cmd.env(key, value);
        }
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn command: {}", e))?;

    // Record the running PID, then clear it once the child exits (keeping the
    // sticky port assignment for next time).
    if let Some(r) = &registration {
        track(r, Some(child.id()));
    }
    let status = child.wait()?;
    if let Some(r) = &registration {
        track(r, None);
    }

    if !status.success() {
        eprintln!("Command exited with: {:?}", status.code());
    }
    Ok(())
}
