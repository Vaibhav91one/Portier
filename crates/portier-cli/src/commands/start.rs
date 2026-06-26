use std::process::Stdio;

use clap::Args;

use crate::output;

#[derive(Args, Debug)]
pub struct StartArgs {
    /// Skip confirmation prompts
    #[arg(long, short)]
    pub yes: bool,
    /// Dry run (show what would change without applying)
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: StartArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    output::print_success("Portier - scanning project...");

    // 1. Detect
    let detection = libportier::detector::detect_stack(&cwd)?;
    output::print_success(format!("Detected stack: {}", detection.stack));

    // 2. Load or infer config
    let (config, config_inferred) = match libportier::config::ProjectConfig::load(&cwd)? {
        Some(c) => (c, false),
        None => {
            let dir_name = cwd
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".into());
            let mut config =
                libportier::config::ProjectConfig::new(dir_name, detection.stack.to_string());
            let env_ports = libportier::config::extract_env_port(&cwd)?;
            for (key, port) in &env_ports {
                config.ports.insert(
                    key.clone(),
                    libportier::config::PortConfig {
                        preferred: *port,
                        assigned: None,
                    },
                );
            }
            (config, true)
        }
    };

    // 3. Scan
    let statuses = libportier::scanner::scan()?;
    let conflicts: Vec<_> = statuses.iter().filter(|s| s.is_conflict).collect();
    if !conflicts.is_empty() {
        for c in &conflicts {
            output::print_warning(format!(
                "Port {} is in use by: {}",
                c.port,
                c.process_names.join(", ")
            ));
        }
    }

    // 4. Assign
    let registry = libportier::registry::Registry::load()?;
    let assigner = libportier::assigner::Assigner::new(registry.clone());
    let preferred_ports: std::collections::HashMap<String, u16> = config
        .ports
        .iter()
        .map(|(k, v)| (k.clone(), v.preferred))
        .collect();

    let assignments = assigner.allocate(&preferred_ports, false, None)?;

    // 5. Show results
    for (service, port) in &assignments {
        let preferred = preferred_ports.get(service).unwrap_or(port);
        output::print_diff(service, &preferred.to_string(), &port.to_string());
    }

    // 5b. Prompt for confirmation unless --yes is given
    if !args.dry_run && !args.yes {
        print!("Apply changes? [Y/n] ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_lowercase();
        if trimmed == "n" || trimmed == "no" {
            output::print_success("Skipped.");
            return Ok(());
        }
    }

    // 6. Apply to config files
    let config_files = libportier::config::find_config_files(&cwd, &config.stack);
    if args.dry_run {
        let diffs = libportier::rewriter::preview_assignments(&assignments, &config_files, &cwd)?;
        if diffs.is_empty() {
            output::print_success("No config file changes.");
        } else {
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
        let snapshot = libportier::rewriter::apply_assignments(&assignments, &config_files, &cwd)?;
        output::print_success(format!("Snapshot saved: {}", snapshot.id));
        if config_inferred {
            config.save(&cwd)?;
        }
    }

    // 7. Save updated registry and launch services
    if !args.dry_run {
        let path_key = cwd.to_string_lossy().to_string();

        // Warn if cwd doesn't match the project's registered worktree
        if let Some(project) = registry.get_project(&path_key) {
            if let Some(ref wt) = project.worktree {
                if wt != &path_key {
                    output::print_warning(format!(
                        "cwd differs from project worktree '{}'",
                        wt
                    ));
                }
            }
        }

        libportier::registry::Registry::update(|reg| {
            if let Some(project) = reg.get_project_mut(&path_key) {
                for (service, port) in &assignments {
                    if let Some(entry) = project.services.get_mut(service) {
                        entry.assigned = *port;
                    }
                }

                // Launch services that have a command configured
                let mut launched_any = false;
                for (name, svc_config) in &config.services {
                    if let Some(command) = &svc_config.command {
                        match std::process::Command::new("sh")
                            .arg("-c")
                            .arg(command)
                            .stdout(Stdio::inherit())
                            .stderr(Stdio::inherit())
                            .current_dir(&cwd)
                            .spawn()
                        {
                            Ok(child) => {
                                let pid = child.id();
                                if let Some(entry) = project.services.get_mut(name) {
                                    entry.pid = Some(pid);
                                }
                                output::print_success(format!("Started {} (PID {}).", name, pid));
                                launched_any = true;
                            }
                            Err(e) => {
                                output::print_error(format!("Failed to start {}: {}", name, e));
                            }
                        }
                    }
                }

                if !launched_any {
                    output::print_success("Port assignments written to config files. Run your project's start command to launch services.");
                }
            }
            Ok(())
        })?;
    }

    output::print_success("Done.");
    Ok(())
}
