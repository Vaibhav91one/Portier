use clap::Args;
use std::process::Command;

use crate::output;

#[derive(Args, Debug)]
pub struct StopArgs {
    /// Project path or name (default: current directory)
    pub name: Option<String>,
}

pub fn run(args: StopArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let path_key = args
        .name
        .unwrap_or_else(|| cwd.to_string_lossy().to_string());

    let registry = libportier::registry::Registry::load()?;

    // Find project by path or name, keeping the registry key
    let (reg_key, entry) = if let Some(e) = registry.projects.get(&path_key) {
        (path_key.clone(), e.clone())
    } else if let Some((k, e)) = registry.find_by_name(&path_key) {
        (k, e.clone())
    } else {
        output::print_warning(format!("No project found for '{}'.", path_key));
        return Ok(());
    };

    // ponytail: use entry.worktree (or reg_key as fallback) for any file
    //           operations on the project root (e.g. config rewrites).
    let pids: Vec<u32> = entry.services.values().filter_map(|s| s.pid).collect();

    for &pid in &pids {
        output::print_success(format!("Stopping PID {}...", pid));
        #[cfg(unix)]
        let _ = Command::new("kill").arg(pid.to_string()).status();
        #[cfg(windows)]
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string()])
            .status();
    }

    // Update registry - clear PIDs using the registry key directly
    libportier::registry::Registry::update(|reg| {
        if let Some(project) = reg.get_project_mut(&reg_key) {
            for service in project.services.values_mut() {
                service.pid = None;
            }
        }
        Ok(())
    })?;
    output::print_success(format!("Stopped {} services.", entry.services.len()));

    Ok(())
}
