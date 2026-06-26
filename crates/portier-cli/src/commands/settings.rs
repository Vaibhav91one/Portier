use clap::Args;

use crate::output;

#[derive(Args, Debug)]
pub struct SettingsArgs {
    /// Write a default config.toml (won't overwrite an existing one)
    #[arg(long)]
    pub init: bool,
    /// Show the current effective settings (default)
    #[arg(long)]
    pub show: bool,
}

pub fn run(args: SettingsArgs) -> anyhow::Result<()> {
    let path = libportier::Settings::path()
        .ok_or_else(|| anyhow::anyhow!("could not resolve home directory"))?;

    if args.init {
        if path.exists() {
            output::print_warning(format!("Settings already exist at {}", path.display()));
            output::print_hint("Edit that file, or delete it and re-run to reset.");
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, libportier::settings::EXAMPLE_TOML)?;
            output::print_success(format!("Wrote default settings to {}", path.display()));
        }
        return Ok(());
    }

    // Default action (and explicit --show): print effective settings.
    let s = libportier::Settings::load();
    let r = s.port_range();
    output::print_success("Effective settings:");
    if path.exists() {
        println!("  config file         {}", path.display());
    } else {
        println!("  config file         (none — using defaults)");
    }
    println!("  port range          {}–{}", r.start, r.end);
    println!("  prefer_consecutive  {}", s.preferences.prefer_consecutive);
    println!("  avoid_well_known    {}", s.preferences.avoid_well_known);
    if !path.exists() {
        output::print_hint("Create an editable config with: portier settings --init");
    }
    Ok(())
}
