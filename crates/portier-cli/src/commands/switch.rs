use clap::Args;
use std::collections::HashMap;
use std::process::Command;

use crate::output;

#[derive(Args, Debug)]
pub struct SwitchArgs {
    /// Target project name
    pub target: String,
}

fn kill_pid(pid: u32) {
    #[cfg(unix)]
    let _ = Command::new("kill").arg(pid.to_string()).status();
    #[cfg(windows)]
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .status();
}

pub fn run(args: SwitchArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let registry = libportier::registry::Registry::load()?;

    let current_path = cwd.to_string_lossy().to_string();

    // 1. Stop current project's services
    if let Some(entry) = registry.get_project(&current_path) {
        output::print_success(format!("Stopping current project '{}'...", entry.name));
        for (svc, svc_entry) in &entry.services {
            if let Some(pid) = svc_entry.pid {
                output::print_success(format!("  Stopping {} (PID {})...", svc, pid));
                kill_pid(pid);
            }
        }
    }

    // 2. Find target project by name
    let target = registry.find_by_name(&args.target);
    let (target_path, target_entry) = match target {
        Some(t) => t,
        None => {
            output::print_warning(format!("Project '{}' not found in registry.", args.target));
            output::print_success("Use 'portier link' to register it first.");
            return Ok(());
        }
    };

    output::print_success(format!("Switching to '{}' at {}", target_entry.name, target_path));

    // 3. Free ports for current project, assign ports for target
    // Collect target preferred ports
    let preferred_ports: HashMap<String, u16> = target_entry
        .services
        .iter()
        .map(|(k, v)| (k.clone(), v.preferred))
        .collect();

    let assigner = libportier::assigner::Assigner::new(registry.clone());
    let assignments = assigner.allocate(&preferred_ports, false, None)?;

    // 4. Show assignments
    for (service, port) in &assignments {
        let preferred = preferred_ports.get(service).unwrap_or(port);
        if port == preferred {
            output::print_success(format!("{} -> {} (free)", service, port));
        } else {
            output::print_warning(format!("{} {} -> {} (reassigned)", service, preferred, port));
        }
    }

    // 5. Apply to config files
    let target_root = std::path::Path::new(&target_path);
    let config_files = libportier::config::find_config_files(target_root, &target_entry.stack);
    let snapshot = libportier::rewriter::apply_assignments(&assignments, &config_files, target_root)?;
    output::print_success(format!("Snapshot saved: {}", snapshot.id));

    // 6. Update registry — clear all PIDs, set target assignments
    libportier::registry::Registry::update(|reg| {
        for (_, entry) in &mut reg.projects {
            for service in entry.services.values_mut() {
                service.pid = None;
            }
        }
        if let Some(project) = reg.get_project_mut(&target_path) {
            for (service, port) in &assignments {
                if let Some(entry) = project.services.get_mut(service) {
                    entry.assigned = *port;
                }
            }
        }
        Ok(())
    })?;

    output::print_success("Switch complete.");
    Ok(())
}
