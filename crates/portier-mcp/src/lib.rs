//! Portier MCP server, built on the official Rust SDK (`rmcp`).
//!
//! Exposes typed, structured tools over stdio so coding agents (Claude Code,
//! Cursor) can detect and resolve port conflicts. Every tool returns a
//! `Json<T>` whose schema is advertised in `tools/list` and whose value lands in
//! the response's `structuredContent`.

use std::collections::{HashMap, HashSet};

use rmcp::{
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Output DTOs (typed structured content)
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema, Clone)]
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

#[derive(Serialize, schemars::JsonSchema)]
struct ScanResult {
    total_ports: usize,
    conflict_count: usize,
    conflicts: Vec<PortInfo>,
    ports: Vec<PortInfo>,
}

#[derive(Serialize, schemars::JsonSchema)]
struct PortAssignment {
    port: u16,
    project: String,
    service: String,
    pid: Option<u32>,
}

#[derive(Serialize, schemars::JsonSchema)]
struct Assignment {
    service: String,
    preferred: u16,
    assigned: u16,
}

#[derive(Serialize, schemars::JsonSchema)]
struct HealResult {
    project: String,
    stack: String,
    conflicts_before: usize,
    assignments: Vec<Assignment>,
    config_files_written: Vec<String>,
    registry_updated: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
struct AllocateResult {
    ports: Vec<u16>,
}

#[derive(Serialize, schemars::JsonSchema)]
struct PortMapResult {
    assignments: Vec<PortAssignment>,
}

// ---------------------------------------------------------------------------
// Tool argument structs
// ---------------------------------------------------------------------------

#[derive(Deserialize, schemars::JsonSchema)]
struct AutoHealArgs {
    /// Absolute path to the project directory.
    project_path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct AllocateArgs {
    /// Preferred starting port (default 3000).
    #[serde(default)]
    preferred: Option<u16>,
    /// How many ports to allocate (default 1).
    #[serde(default)]
    count: Option<usize>,
}

fn internal<E: std::fmt::Display>(e: E) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Portier {
    // Read by the `#[tool_handler]`-generated dispatch; the lint can't see
    // through the macro expansion.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl Portier {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for Portier {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl Portier {
    #[tool(
        description = "Scan all listening TCP ports and detect conflicts. Returns structured port + conflict arrays."
    )]
    fn scan_conflicts(&self) -> Result<Json<ScanResult>, ErrorData> {
        let statuses = libportier::scanner::scan().map_err(internal)?;
        let ports: Vec<PortInfo> = statuses.iter().map(PortInfo::from).collect();
        let conflicts: Vec<PortInfo> = ports.iter().filter(|p| p.is_conflict).cloned().collect();
        Ok(Json(ScanResult {
            total_ports: ports.len(),
            conflict_count: conflicts.len(),
            conflicts,
            ports,
        }))
    }

    #[tool(
        description = "Allocate free port(s) without side effects — does not touch config files or the registry."
    )]
    fn allocate_port(
        &self,
        Parameters(args): Parameters<AllocateArgs>,
    ) -> Result<Json<AllocateResult>, ErrorData> {
        let preferred = args.preferred.unwrap_or(3000);
        let count = args.count.unwrap_or(1).max(1);

        let statuses = libportier::scanner::scan().map_err(internal)?;
        let mut in_use: HashSet<u16> = statuses.iter().map(|s| s.port).collect();

        let mut ports = Vec::with_capacity(count);
        let mut next = preferred;
        for _ in 0..count {
            let p = libportier::pick_free_port(next, &in_use, None);
            ports.push(p);
            in_use.insert(p);
            next = p.saturating_add(1);
        }
        Ok(Json(AllocateResult { ports }))
    }

    #[tool(description = "Return the registry's port assignments as a structured array.")]
    fn get_port_map(&self) -> Result<Json<PortMapResult>, ErrorData> {
        let registry = libportier::registry::Registry::load().map_err(internal)?;
        let mut assignments: Vec<PortAssignment> = Vec::new();
        for entry in registry.projects.values() {
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
        Ok(Json(PortMapResult { assignments }))
    }

    #[tool(
        description = "Detect the project at a path, allocate free ports for its services, rewrite its config files, and update the registry."
    )]
    fn auto_heal(
        &self,
        Parameters(args): Parameters<AutoHealArgs>,
    ) -> Result<Json<HealResult>, ErrorData> {
        let project_path = args.project_path;
        let project_root = std::path::Path::new(&project_path);
        if !project_root.exists() {
            return Err(ErrorData::invalid_params(
                format!("Project path does not exist: {project_path}"),
                None,
            ));
        }

        let statuses = libportier::scanner::scan().map_err(internal)?;
        let conflicts_before = statuses.iter().filter(|s| s.is_conflict).count();

        let detection = libportier::detector::detect_stack(project_root).map_err(internal)?;
        let registry = libportier::registry::Registry::load().map_err(internal)?;

        let name = project_root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let stack = detection.stack.to_string();

        // Prefer an existing entry found by name, else key on the path.
        let path_key = registry
            .find_by_name(&name)
            .map(|(key, _)| key)
            .unwrap_or_else(|| project_path.clone());
        let is_new = registry.get_project(&path_key).is_none();

        // Preferred ports from existing services + the project's config file.
        let existing = registry.get_project(&path_key);
        let mut preferred_ports: HashMap<String, u16> = HashMap::new();
        if let Some(p) = existing {
            for (svc_name, svc) in &p.services {
                preferred_ports.insert(svc_name.clone(), svc.preferred);
            }
        }
        let worktree_path = existing
            .and_then(|p| p.worktree.clone())
            .unwrap_or_else(|| project_path.clone());
        if let Some(config) =
            libportier::config::ProjectConfig::load(std::path::Path::new(&worktree_path))
                .map_err(internal)?
        {
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
            worktree: Some(project_path.clone()),
        };

        let mut assignments_out: Vec<Assignment> = Vec::new();
        let mut config_files_written: Vec<String> = Vec::new();
        let mut registry_updated = false;

        if !preferred_ports.is_empty() {
            // Allocate against a snapshot that includes the (possibly new) project.
            let mut snapshot = registry.clone();
            if is_new {
                snapshot.add_project(path_key.clone(), new_entry());
            }
            let assigner = libportier::Assigner::new(snapshot);
            let assignments = assigner
                .allocate(&preferred_ports, false, None)
                .map_err(internal)?;

            for (service, port) in &assignments {
                let preferred = *preferred_ports.get(service).unwrap_or(port);
                assignments_out.push(Assignment {
                    service: service.clone(),
                    preferred,
                    assigned: *port,
                });
            }

            // Rewrite config files (snapshotted by the rewriter).
            let config_files =
                libportier::config::find_config_files(std::path::Path::new(&worktree_path), &stack);
            if !config_files.is_empty() {
                libportier::rewriter::apply_assignments(
                    &assignments,
                    &config_files,
                    std::path::Path::new(&worktree_path),
                )
                .map_err(internal)?;
                config_files_written = config_files;
            }

            // Commit the registry mutation atomically.
            let preferred_ports_c = preferred_ports.clone();
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
                                    preferred: *preferred_ports_c.get(service).unwrap_or(port),
                                    assigned: *port,
                                    pid: None,
                                },
                            );
                        }
                    }
                }
                Ok(())
            })
            .map_err(internal)?;
            registry_updated = true;
        } else if is_new {
            libportier::registry::Registry::update(|reg| {
                reg.add_project(path_key.clone(), new_entry());
                Ok(())
            })
            .map_err(internal)?;
            registry_updated = true;
        }

        Ok(Json(HealResult {
            project: name.clone(),
            stack: stack.clone(),
            conflicts_before,
            assignments: assignments_out,
            config_files_written,
            registry_updated,
        }))
    }
}

#[tool_handler]
impl ServerHandler for Portier {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("portier", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Portier resolves local dev-server port conflicts. Use scan_conflicts to inspect, \
                 allocate_port for a free port, and auto_heal to assign + rewrite a project's configs.",
            )
    }
}

/// Run the MCP server over stdio. Blocks until the client disconnects.
pub fn run_mcp() {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("portier mcp: failed to start runtime: {e}");
            return;
        }
    };
    runtime.block_on(async {
        match Portier::new().serve(stdio()).await {
            Ok(service) => {
                let _ = service.waiting().await;
            }
            Err(e) => eprintln!("portier mcp: failed to start server: {e}"),
        }
    });
}
