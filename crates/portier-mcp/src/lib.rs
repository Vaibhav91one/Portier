use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use schemars::{schema_for, JsonSchema};
use serde::Serialize;
use serde_json::{json, Value};

/// A tool result: a human-readable `text` summary plus machine-readable
/// `structured` JSON. Agents read `structured`; `text` is the back-compat
/// summary for clients that only render text content.
struct ToolOutput {
    text: String,
    structured: Value,
}

#[derive(Serialize, JsonSchema, Clone)]
struct PortInfo {
    port: u16,
    pids: Vec<u32>,
    process_names: Vec<String>,
    is_conflict: bool,
}

impl From<&libportier::PortStatus> for PortInfo {
    fn from(s: &libportier::PortStatus) -> Self {
        PortInfo {
            port: s.port,
            pids: s.pids.clone(),
            process_names: s.process_names.clone(),
            is_conflict: s.is_conflict,
        }
    }
}

#[derive(Serialize, JsonSchema)]
struct ScanResult {
    total_ports: usize,
    conflict_count: usize,
    conflicts: Vec<PortInfo>,
    ports: Vec<PortInfo>,
}

#[derive(Serialize, JsonSchema)]
struct PortAssignment {
    port: u16,
    project: String,
    service: String,
    pid: Option<u32>,
}

#[derive(Serialize, JsonSchema)]
struct Assignment {
    service: String,
    preferred: u16,
    assigned: u16,
}

#[derive(Serialize, JsonSchema)]
struct HealResult {
    project: String,
    stack: String,
    conflicts_before: usize,
    assignments: Vec<Assignment>,
    config_files_written: Vec<String>,
    registry_updated: bool,
}

#[derive(Serialize, JsonSchema)]
struct AllocateResult {
    ports: Vec<u16>,
}

/// Run the MCP server over stdin/stdout using newline-delimited JSON-RPC.
pub fn run_mcp() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,  // EOF
            Ok(_) => {}
            Err(e) => {
                let _ = respond_error(
                    &mut stdout.lock(),
                    json!(null),
                    -32700,
                    &format!("Parse error: {e}"),
                );
                break;
            }
        }

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = respond_error(
                    &mut stdout.lock(),
                    json!(null),
                    -32700,
                    &format!("Parse error: {e}"),
                );
                continue;
            }
        };

        let method = req["method"].as_str().unwrap_or("");
        let id = req.get("id").cloned().unwrap_or(json!(null));

        let mut out = stdout.lock();
        match method {
            "initialize" => {
                respond(
                    &mut out,
                    id,
                    json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": { "tools": {} }
                    }),
                );
            }
            "tools/list" => {
                let tools = list_tools();
                respond(&mut out, id, json!({ "tools": tools }));
            }
            "tools/call" => {
                let name = req["params"]["name"].as_str().unwrap_or("");
                let empty = json!({});
                let args = req["params"].get("arguments").unwrap_or(&empty);
                match call_tool(name, args) {
                    Ok(result) => {
                        respond(
                            &mut out,
                            id,
                            json!({
                                "content": [{"type": "text", "text": result.text}],
                                "structuredContent": result.structured,
                                "isError": false
                            }),
                        );
                    }
                    Err(e) => {
                        respond_error(&mut out, id, -32603, &format!("Internal error: {e}"));
                    }
                }
            }
            "notifications/initialized" => {
                // No response needed for notifications
            }
            _ => {
                respond_error(&mut out, id, -32601, &format!("Method not found: {method}"));
            }
        }
    }
}

/// Build the tool definitions list for `tools/list`.
fn list_tools() -> Vec<Value> {
    let schema = |s| serde_json::to_value(s).unwrap_or_else(|_| json!({}));
    vec![
        json!({
            "name": "scan_conflicts",
            "description": "Scan the system for all listening TCP ports and detect conflicts. Returns structured port + conflict arrays.",
            "inputSchema": { "type": "object", "properties": {} },
            "outputSchema": schema(schema_for!(ScanResult))
        }),
        json!({
            "name": "auto_heal",
            "description": "Detect the project at a path, allocate free ports for its services, rewrite its config files, and update the registry. Returns the assignments made.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_path": {
                        "type": "string",
                        "description": "Absolute path to the project directory"
                    }
                },
                "required": ["project_path"]
            },
            "outputSchema": schema(schema_for!(HealResult))
        }),
        json!({
            "name": "get_port_map",
            "description": "Return the registry's port assignments as a structured array of {port, project, service, pid}.",
            "inputSchema": { "type": "object", "properties": {} },
            "outputSchema": schema(schema_for!(Vec<PortAssignment>))
        }),
        json!({
            "name": "allocate_port",
            "description": "Allocate free port(s) without side effects — does not touch config files or the registry. Useful before starting a dev server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "preferred": { "type": "integer", "description": "Preferred starting port (default 3000)" },
                    "count": { "type": "integer", "description": "How many ports to allocate (default 1)" }
                }
            },
            "outputSchema": schema(schema_for!(AllocateResult))
        }),
        json!({
            "name": "run_command",
            "description": "DEPRECATED — prefer the `portier run` CLI, which allocates and injects a free port. Runs a shell command with pre/post port scanning.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to run"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory for the command (optional)"
                    }
                },
                "required": ["command"]
            }
        }),
    ]
}

/// Dispatch a tool call to the appropriate handler.
fn call_tool(name: &str, args: &Value) -> anyhow::Result<ToolOutput> {
    match name {
        "scan_conflicts" => tool_scan_conflicts(),
        "auto_heal" => tool_auto_heal(args),
        "get_port_map" => tool_get_port_map(),
        "allocate_port" => tool_allocate_port(args),
        "run_command" => tool_run_command(args),
        _ => anyhow::bail!("Unknown tool: {name}"),
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

/// Scan for port conflicts and return structured port + conflict arrays.
fn tool_scan_conflicts() -> anyhow::Result<ToolOutput> {
    let statuses = libportier::scanner::scan()?;
    let ports: Vec<PortInfo> = statuses.iter().map(PortInfo::from).collect();
    let conflicts: Vec<PortInfo> = ports.iter().filter(|p| p.is_conflict).cloned().collect();

    let text = if conflicts.is_empty() {
        format!("{} listening ports, no conflicts.", ports.len())
    } else {
        format!(
            "{} listening ports, {} conflict(s).",
            ports.len(),
            conflicts.len()
        )
    };

    let result = ScanResult {
        total_ports: ports.len(),
        conflict_count: conflicts.len(),
        conflicts,
        ports,
    };
    Ok(ToolOutput {
        text,
        structured: serde_json::to_value(result)?,
    })
}

/// Auto-heal: scan ports, detect project from path, assign free ports, update registry.
fn tool_auto_heal(args: &Value) -> anyhow::Result<ToolOutput> {
    let project_path = args["project_path"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: project_path"))?;

    let project_root = std::path::Path::new(project_path);
    if !project_root.exists() {
        anyhow::bail!("Project path does not exist: {project_path}");
    }

    let mut lines = Vec::new();

    // Structured-output accumulators.
    let mut assignments_out: Vec<Assignment> = Vec::new();
    let mut config_files_written: Vec<String> = Vec::new();
    let mut registry_updated = false;

    // Step 1: scan current state
    let statuses = libportier::scanner::scan()?;
    let current_conflicts: Vec<_> = statuses.iter().filter(|s| s.is_conflict).collect();
    let conflicts_before = current_conflicts.len();
    if current_conflicts.is_empty() {
        lines.push("No port conflicts detected on the system.".to_string());
    } else {
        lines.push(format!(
            "Found {} conflict(s) before healing:",
            current_conflicts.len()
        ));
        for c in &current_conflicts {
            lines.push(format!("  Port {} — {}", c.port, c.process_names.join(", ")));
        }
    }

    // Step 2: detect project stack
    let detection = libportier::detector::detect_stack(project_root)?;
    lines.push(format!(
        "Detected project: {} ({}) at {}",
        project_root
            .file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or_else(|| project_path.into()),
        detection.stack,
        project_root.display()
    ));

    // Step 3: load config and assign ports.
    //
    // We compute the plan against a read-only snapshot, then commit the
    // registry mutation in a single `Registry::update` transaction so a
    // concurrent CLI/daemon write can't clobber it.
    let registry = libportier::registry::Registry::load()?;

    let name = project_root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let stack = detection.stack.to_string();

    // Resolve the registry key: prefer an existing entry found by name, else
    // key on the project path (registering a fresh entry if absent).
    let path_key = registry
        .find_by_name(&name)
        .map(|(key, _)| key)
        .unwrap_or_else(|| project_path.to_string());
    let is_new = registry.get_project(&path_key).is_none();

    // Build preferred ports from the existing project's services + config file.
    let existing = registry.get_project(&path_key);
    let mut preferred_ports: HashMap<String, u16> = HashMap::new();
    if let Some(p) = existing {
        for (svc_name, svc) in &p.services {
            preferred_ports.insert(svc_name.clone(), svc.preferred);
        }
    }
    let worktree_path = existing
        .and_then(|p| p.worktree.clone())
        .unwrap_or_else(|| project_path.to_string());
    if let Some(config) = libportier::config::ProjectConfig::load(std::path::Path::new(&worktree_path))? {
        for (port_name, port_cfg) in &config.ports {
            preferred_ports
                .entry(port_name.clone())
                .or_insert(port_cfg.preferred);
        }
    }

    let new_entry = || libportier::registry::ProjectEntry {
        name: name.clone(),
        stack: stack.clone(),
        services: HashMap::new(),
        linked: false,
        worktree: Some(project_path.to_string()),
    };

    if !preferred_ports.is_empty() {
        // Allocate against a snapshot that includes the (possibly new) project.
        let mut snapshot = registry.clone();
        if is_new {
            snapshot.add_project(path_key.clone(), new_entry());
        }
        let assigner = libportier::Assigner::new(snapshot);
        let assignments = assigner.allocate(&preferred_ports, false, None)?;

        lines.push("Port assignments:".to_string());
        for (service, port) in &assignments {
            let preferred = *preferred_ports.get(service).unwrap_or(port);
            lines.push(format!("    {service}: {} → {}", preferred, port));
            assignments_out.push(Assignment {
                service: service.clone(),
                preferred,
                assigned: *port,
            });
        }

        // Apply config changes (side effect, snapshotted by the rewriter).
        let config_files =
            libportier::config::find_config_files(std::path::Path::new(&worktree_path), &stack);
        if !config_files.is_empty() {
            let _ = libportier::rewriter::apply_assignments(
                &assignments,
                &config_files,
                std::path::Path::new(&worktree_path),
            )?;
            lines.push(format!("Config files updated: {}", config_files.join(", ")));
            config_files_written = config_files.clone();
        }

        // Commit the registry mutation atomically.
        libportier::registry::Registry::update(|reg| {
            if is_new {
                reg.add_project(path_key.clone(), new_entry());
            }
            if let Some(proj) = reg.get_project_mut(&path_key) {
                for (service, port) in &assignments {
                    if let Some(svc_entry) = proj.services.get_mut(service) {
                        svc_entry.assigned = *port;
                    } else {
                        proj.services.insert(
                            service.clone(),
                            libportier::registry::ServiceEntry {
                                preferred: *preferred_ports.get(service).unwrap_or(port),
                                assigned: *port,
                                pid: None,
                            },
                        );
                    }
                }
            }
            Ok(())
        })?;
        lines.push("Registry updated.".to_string());
        registry_updated = true;
    } else {
        if is_new {
            libportier::registry::Registry::update(|reg| {
                reg.add_project(path_key.clone(), new_entry());
                Ok(())
            })?;
            registry_updated = true;
        }
        lines.push("No ports configured for this project.".to_string());
    }

    let result = HealResult {
        project: name.clone(),
        stack: stack.clone(),
        conflicts_before,
        assignments: assignments_out,
        config_files_written,
        registry_updated,
    };
    Ok(ToolOutput {
        text: lines.join("\n"),
        structured: serde_json::to_value(result)?,
    })
}

/// Return the registry's port assignments as a structured array.
fn tool_get_port_map() -> anyhow::Result<ToolOutput> {
    let registry = libportier::registry::Registry::load()?;
    let mut assignments: Vec<PortAssignment> = Vec::new();

    for (_path, entry) in &registry.projects {
        for (svc_name, svc) in &entry.services {
            assignments.push(PortAssignment {
                port: svc.assigned,
                project: entry.name.clone(),
                service: svc_name.clone(),
                pid: svc.pid,
            });
        }
    }
    assignments.sort_by_key(|a| a.port);

    let text = format!("{} assigned port(s) in registry.", assignments.len());
    Ok(ToolOutput {
        text,
        structured: serde_json::to_value(&assignments)?,
    })
}

/// Allocate free port(s) without side effects (no config/registry writes).
fn tool_allocate_port(args: &Value) -> anyhow::Result<ToolOutput> {
    let preferred = args["preferred"].as_u64().unwrap_or(3000) as u16;
    let count = args["count"].as_u64().unwrap_or(1).max(1) as usize;

    let statuses = libportier::scanner::scan()?;
    let mut in_use: std::collections::HashSet<u16> =
        statuses.iter().map(|s| s.port).collect();

    let mut ports = Vec::with_capacity(count);
    let mut next = preferred;
    for _ in 0..count {
        let p = libportier::pick_free_port(next, &in_use, None);
        ports.push(p);
        in_use.insert(p); // don't hand out the same port twice
        next = p.saturating_add(1);
    }

    let text = format!("Allocated port(s): {ports:?}");
    Ok(ToolOutput {
        text,
        structured: serde_json::to_value(AllocateResult { ports })?,
    })
}

/// Run a shell command with pre/post port scanning (deprecated; see `portier run`).
fn tool_run_command(args: &Value) -> anyhow::Result<ToolOutput> {
    let command = args["command"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: command"))?;
    let cwd = args["cwd"].as_str();

    let mut lines = Vec::new();

    // Scan before
    let before = libportier::scanner::scan()?;
    let before_conflicts: Vec<_> = before.iter().filter(|s| s.is_conflict).collect();
    if before_conflicts.is_empty() {
        lines.push("Pre-scan: no port conflicts.".to_string());
    } else {
        lines.push(format!(
            "Pre-scan: {} conflict(s) detected.",
            before_conflicts.len()
        ));
    }

    // Prepare command
    let shell_cmd = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };
    let shell_flag = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    let mut child = std::process::Command::new(shell_cmd);
    child.arg(shell_flag).arg(command);
    if let Some(dir) = cwd {
        child.current_dir(dir);
    }

    let output = child.output()?;

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);

    if !stdout_str.is_empty() {
        lines.push("--- stdout ---".to_string());
        lines.push(stdout_str.trim().to_string());
    }
    if !stderr_str.is_empty() {
        lines.push("--- stderr ---".to_string());
        lines.push(stderr_str.trim().to_string());
    }
    let exit_code = output.status.code().unwrap_or(-1);
    lines.push(format!("Exit code: {exit_code}"));

    // Scan after
    let after = libportier::scanner::scan()?;
    let after_conflicts: Vec<_> = after.iter().filter(|s| s.is_conflict).collect();
    if after_conflicts.is_empty() {
        lines.push("Post-scan: no port conflicts.".to_string());
    } else {
        lines.push(format!(
            "Post-scan: {} conflict(s) detected.",
            after_conflicts.len()
        ));
        for c in &after_conflicts {
            lines.push(format!("  Port {} — {}", c.port, c.process_names.join(", ")));
        }
    }

    let structured = json!({
        "exit_code": exit_code,
        "conflicts_before": before_conflicts.len(),
        "conflicts_after": after_conflicts.len(),
        "stdout": stdout_str.trim(),
        "stderr": stderr_str.trim(),
    });
    Ok(ToolOutput {
        text: lines.join("\n"),
        structured,
    })
}

// ---------------------------------------------------------------------------
// JSON-RPC helpers
// ---------------------------------------------------------------------------

fn respond(out: &mut io::StdoutLock, id: Value, result: Value) {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    let _ = writeln!(out, "{}", msg);
    let _ = out.flush();
}

fn respond_error(out: &mut io::StdoutLock, id: Value, code: i32, message: &str) {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    });
    let _ = writeln!(out, "{}", msg);
    let _ = out.flush();
}
