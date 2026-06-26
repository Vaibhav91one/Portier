use clap::{Args, Subcommand};

use crate::output;

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Initialize portier.json in the current directory
    Init {
        /// Project name (default: directory name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Show current config
    Show,
    /// Set a config value (key=value)
    Set {
        /// Key=value pair, e.g. ports.web.preferred=4000
        key_value: String,
    },
}

pub fn run(args: ConfigArgs) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;

    match args.action {
        Some(ConfigAction::Init { name }) => {
            let dir_name = cwd
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".into());
            let project_name = name.unwrap_or(dir_name);

            let detection = libportier::detector::detect_stack(&cwd)?;

            let mut config = libportier::config::ProjectConfig::new(project_name, detection.stack.to_string());

            // Try to extract ports from .env
            let env_ports = libportier::config::extract_env_port(&cwd)?;
            for (key, port) in env_ports {
                config.ports.insert(
                    key,
                    libportier::config::PortConfig {
                        preferred: port,
                        assigned: None,
                    },
                );
            }

            config.save(&cwd)?;
            output::print_success(format!("Created portier.json for {} ({})", config.name, config.stack));
        }
        Some(ConfigAction::Show) => {
            match libportier::config::ProjectConfig::load(&cwd)? {
                Some(config) => {
                    output::print_json(&config);
                }
                None => {
                    output::print_warning("No portier.json found in current directory.");
                    output::print_success("Run 'portier config --init' to create one.");
                }
            }
        }
        Some(ConfigAction::Set { key_value }) => {
            let mut config = libportier::config::ProjectConfig::load(&cwd)?
                .ok_or_else(|| anyhow::anyhow!("No portier.json found. Run 'portier config --init' first."))?;

            let parts: Vec<&str> = key_value.splitn(2, '=').collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid format. Use key=value, e.g. ports.web.preferred=4000");
            }

            let key = parts[0].trim();
            let val = parts[1].trim();

            // Support: ports.<name>.preferred, ports.<name>.assigned
            if let Some(rest) = key.strip_prefix("ports.") {
                let sub: Vec<&str> = rest.splitn(2, '.').collect();
                if sub.len() == 2 {
                    let port_name = sub[0].to_string();
                    let field = sub[1];
                    let port_val: u16 = val.parse()
                        .map_err(|_| anyhow::anyhow!("Invalid port number: {}", val))?;

                    let entry = config.ports.entry(port_name.clone())
                        .or_insert_with(|| libportier::config::PortConfig {
                            preferred: port_val,
                            assigned: None,
                        });
                    match field {
                        "preferred" => entry.preferred = port_val,
                        "assigned" => entry.assigned = Some(port_val),
                        _ => anyhow::bail!("Unknown port field: {}. Use 'preferred' or 'assigned'.", field),
                    }
                }
            }

            config.save(&cwd)?;
            output::print_success("Updated portier.json");
        }
        None => {
            output::print_success("Usage: portier config [--init | --show | --set key=value]");
        }
    }

    Ok(())
}
