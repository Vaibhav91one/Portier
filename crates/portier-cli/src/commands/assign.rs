use clap::Args;
use std::collections::HashMap;

use crate::output;

#[derive(Args, Debug)]
pub struct AssignArgs {
    /// Project name or path
    pub name: String,
    /// Port range (e.g. 3000-4000)
    #[arg(long)]
    pub range: Option<String>,
    /// Dry run (show what would change without applying)
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: AssignArgs) -> anyhow::Result<()> {
    let registry = libportier::registry::Registry::load()?;

    // Find project entry by name or by path key
    let entry = registry
        .find_by_name(&args.name)
        .map(|(_, e)| e)
        .or_else(|| registry.projects.get(&args.name))
        .cloned()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Project '{}' not found in registry. Use 'portier link' to register it.",
                args.name
            )
        })?;

    // Parse optional range
    let range: Option<std::ops::Range<u16>> = match &args.range {
        Some(r) => {
            let parts: Vec<&str> = r.splitn(2, '-').collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid range '{}'. Use START-END, e.g. 3000-4000", r);
            }
            let start: u16 = parts[0]
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid range start '{}'", parts[0]))?;
            let end: u16 = parts[1]
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid range end '{}'", parts[1]))?;
            if start >= end {
                anyhow::bail!("Invalid range: start ({}) must be less than end ({})", start, end);
            }
            Some(start..end)
        }
        None => None,
    };

    // Build preferred ports map from the registry entry's services
    let mut preferred_ports: HashMap<String, u16> = HashMap::new();
    for (svc_name, svc) in &entry.services {
        preferred_ports.insert(svc_name.clone(), svc.preferred);
    }

    // Also try to load project config for more port info
    if let Some(worktree) = &entry.worktree {
        let root = std::path::Path::new(worktree);
        if let Some(config) = libportier::config::ProjectConfig::load(root)? {
            for (name, port_cfg) in &config.ports {
                if !preferred_ports.contains_key(name) {
                    preferred_ports.insert(name.clone(), port_cfg.preferred);
                }
            }
        }
    }

    if preferred_ports.is_empty() {
        output::print_warning(format!("No ports configured for project '{}'.", entry.name));
        output::print_warning("Use 'portier config init' in the project directory to set up ports.");
        return Ok(());
    }

    if args.dry_run {
        output::print_success(format!("Dry run for '{}' ({}):", entry.name, entry.stack));
    } else {
        output::print_success(format!("Assigning ports for '{}' ({}):", entry.name, entry.stack));
    }

    // Global settings supply the default range + consecutive preference; an
    // explicit --range still wins.
    let settings = libportier::Settings::load();
    let range = range.or_else(|| Some(settings.port_range()));
    let assigner = libportier::Assigner::new(registry.clone());
    let assignments =
        assigner.allocate(&preferred_ports, settings.preferences.prefer_consecutive, range)?;

    // Print results
    for (service, port) in &assignments {
        let preferred = preferred_ports.get(service).unwrap_or(port);
        output::print_diff(service, &preferred.to_string(), &port.to_string());
    }

    // 6. Apply to config files
    if let Some(ref worktree) = entry.worktree {
        let root = std::path::Path::new(worktree);
        let config_files = libportier::config::find_config_files(root, &entry.stack);
        if args.dry_run {
            let diffs = libportier::rewriter::preview_assignments(&assignments, &config_files, root)?;
            if !diffs.is_empty() {
                output::print_success("Config file changes (dry run):");
                for (file, file_diffs) in &diffs {
                    println!("    {}:", file);
                    for d in file_diffs {
                        if let libportier::DiffLine::Replaced { old, new } = d {
                            output::print_diff("      ", old, new);
                        }
                    }
                }
            }
        } else {
            let snapshot = libportier::rewriter::apply_assignments(&assignments, &config_files, root)?;
            output::print_success(format!("Snapshot saved: {}", snapshot.id));
        }
    }

    if args.dry_run {
        output::print_success("Dry run — no changes applied.");
        return Ok(());
    }

    // Update registry
    let path_key = registry
        .find_by_name(&args.name)
        .map(|(k, _)| k)
        .or_else(|| {
            if registry.projects.contains_key(&args.name) {
                Some(args.name.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("Project path not found"))?;

    let updated = libportier::registry::Registry::update(|reg| {
        let Some(project) = reg.get_project_mut(&path_key) else {
            return Ok(false);
        };
        for (service, port) in &assignments {
            if let Some(svc_entry) = project.services.get_mut(service) {
                svc_entry.assigned = *port;
            } else {
                project.services.insert(
                    service.clone(),
                    libportier::registry::ServiceEntry {
                        preferred: *preferred_ports.get(service).unwrap_or(port),
                        assigned: *port,
                        pid: None,
                    },
                );
            }
        }
        Ok(true)
    })?;

    if updated {
        output::print_success("Registry updated.");
    }

    Ok(())
}
