use libportier::{PortStatus, ProjectEntry, Registry, ServiceEntry};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct ProjectSummary {
    name: String,
    stack: String,
    path: String,
    services_count: usize,
}

#[derive(Serialize)]
pub struct PortProjectMap {
    pub port: u16,
    pub pids: Vec<u32>,
    pub process_names: Vec<String>,
    pub is_conflict: bool,
    pub project_name: Option<String>,
    pub service_name: Option<String>,
}

#[tauri::command]
fn scan_ports() -> Result<Vec<PortStatus>, String> {
    libportier::scanner::scan().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_projects() -> Result<Vec<ProjectSummary>, String> {
    let reg = Registry::load().map_err(|e| e.to_string())?;
    Ok(reg
        .projects
        .into_iter()
        .map(|(path, entry)| ProjectSummary {
            name: entry.name,
            stack: entry.stack,
            path,
            services_count: entry.services.len(),
        })
        .collect())
}

#[tauri::command]
fn assign_port(project_path: String, service: String, port: u16) -> Result<(), String> {
    Registry::update(|reg| {
        if let Some(proj) = reg.get_project_mut(&project_path) {
            if let Some(svc) = proj.services.get_mut(&service) {
                svc.assigned = port;
            } else {
                proj.services.insert(
                    service.clone(),
                    ServiceEntry {
                        preferred: port,
                        assigned: port,
                        pid: None,
                    },
                );
            }
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn add_project(path_str: String) -> Result<ProjectSummary, String> {
    let root = std::path::Path::new(&path_str);
    let detection = libportier::detector::detect_stack(root).map_err(|e| e.to_string())?;
    let name = root.file_name().unwrap().to_string_lossy().to_string();
    let entry = ProjectEntry {
        name: name.clone(),
        stack: detection.stack.to_string(),
        services: HashMap::new(),
        linked: true,
        worktree: Some(path_str.clone()),
    };
    Registry::update(|reg| {
        reg.add_project(path_str.clone(), entry);
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok(ProjectSummary {
        name,
        stack: detection.stack.to_string(),
        path: path_str,
        services_count: 0,
    })
}

#[tauri::command]
fn remove_project(path_str: String) -> Result<(), String> {
    Registry::update(|reg| {
        reg.remove_project(&path_str);
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_project_detail(path_str: String) -> Result<Option<ProjectEntry>, String> {
    let reg = Registry::load().map_err(|e| e.to_string())?;
    Ok(reg.get_project(&path_str).cloned())
}

#[tauri::command]
fn get_port_project_map() -> Result<Vec<PortProjectMap>, String> {
    let ports = libportier::scanner::scan().map_err(|e| e.to_string())?;
    let registry = Registry::load().ok();

    // Build pid -> (project_name, service_name) from registry tracked PIDs
    let mut pid_map: HashMap<u32, (String, String)> = HashMap::new();
    if let Some(ref reg) = registry {
        for entry in reg.projects.values() {
            for (svc_name, svc) in &entry.services {
                if let Some(pid) = svc.pid {
                    pid_map.insert(pid, (entry.name.clone(), svc_name.clone()));
                }
            }
        }
    }

    let result: Vec<PortProjectMap> = ports
        .into_iter()
        .map(|status| {
            let mut project_name = None;
            let mut service_name = None;

            for pid in &status.pids {
                // 1. Try direct PID match (from portier start tracking)
                if let Some((proj, svc)) = pid_map.get(pid) {
                    project_name = Some(proj.clone());
                    service_name = Some(svc.clone());
                    break;
                }
                // 2. Auto-detect from running process (no registry needed)
                if let Some((name, _root)) = libportier::detector::detect_project_from_pid(*pid) {
                    project_name = Some(name);
                    break;
                }
            }

            PortProjectMap {
                port: status.port,
                pids: status.pids,
                process_names: status.process_names,
                is_conflict: status.is_conflict,
                project_name,
                service_name,
            }
        })
        .collect();

    Ok(result)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scan_ports,
            get_projects,
            assign_port,
            add_project,
            remove_project,
            get_project_detail,
            get_port_project_map
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
