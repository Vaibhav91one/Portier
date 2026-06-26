use clap::Args;
use std::path::PathBuf;

use crate::output;

#[derive(Args, Debug)]
pub struct LinkArgs {
    /// Path to project (default: current dir)
    pub path: Option<String>,
    /// Project name (default: directory name)
    #[arg(long)]
    pub name: Option<String>,
}

pub fn run(args: LinkArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let project_path = args.path.map(PathBuf::from).unwrap_or(cwd);
    let project_name = args.name.unwrap_or_else(|| {
        project_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".into())
    });

    if !project_path.exists() {
        anyhow::bail!("Path does not exist: {}", project_path.display());
    }

    let detection = libportier::detector::detect_stack(&project_path)?;
    output::print_success(format!("Detected stack: {} ({})", detection.stack, detection.config_files.join(", ")));

    // Read existing project config if present
    let existing_config = libportier::config::ProjectConfig::load(&project_path)?;

    let mut services = std::collections::HashMap::new();

    if let Some(config) = existing_config {
        for (name, port_cfg) in &config.ports {
            services.insert(
                name.clone(),
                libportier::registry::ServiceEntry {
                    preferred: port_cfg.preferred,
                    assigned: port_cfg.assigned.unwrap_or(port_cfg.preferred),
                    pid: None,
                },
            );
        }
    }

    let entry = libportier::registry::ProjectEntry {
        name: project_name,
        stack: detection.stack.to_string(),
        services,
        linked: true,
        worktree: Some(project_path.to_string_lossy().to_string()),
    };

    let path_key = project_path.to_string_lossy().to_string();
    libportier::registry::Registry::update(|reg| {
        reg.add_project(path_key.clone(), entry);
        Ok(())
    })?;

    output::print_success("Project linked.");
    output::print_hint("Next: portier start   (assign ports, rewrite configs, launch)");
    Ok(())
}
