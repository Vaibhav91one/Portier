use clap::Args;

use crate::output;

#[derive(Args, Debug)]
pub struct ScanArgs {
    /// Show registered projects
    #[arg(long)]
    pub projects: bool,
    /// JSON output
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: ScanArgs) -> anyhow::Result<()> {
    let statuses = libportier::scanner::scan()?;

    if args.json {
        output::print_json(&statuses);
        return Ok(());
    }

    if statuses.is_empty() {
        output::print_warning("No listening ports found.");
        return Ok(());
    }

    let conflicts: Vec<_> = statuses.iter().filter(|s| s.is_conflict).collect();

    output::print_success(format!("{} ports listening.", statuses.len()));

    if conflicts.is_empty() {
        output::print_success("No conflicts — all clear.");
    } else {
        output::print_warning(format!(
            "{} port conflict{} detected:",
            conflicts.len(),
            if conflicts.len() == 1 { "" } else { "s" }
        ));
        println!();
        let rows: Vec<Vec<String>> = conflicts
            .iter()
            .map(|c| {
                vec![
                    c.port.to_string(),
                    c.process_names.join(", "),
                    format!("{:?}", c.pids),
                ]
            })
            .collect();
        output::print_table(&["PORT", "PROCESSES", "PIDS"], &rows);
        println!();
        output::print_hint("Fix it: portier run -- <your dev command>   (or: portier start)");
    }

    if args.projects {
        let registry = libportier::registry::Registry::load()?;
        if registry.projects.is_empty() {
            println!();
            output::print_hint("No projects registered yet. Add one with: portier link");
        } else {
            println!("\n  Registered projects:");
            for entry in registry.projects.values() {
                println!("    {} ({})", entry.name, entry.stack);
                for (svc, svc_entry) in &entry.services {
                    println!("      {} → {}", svc, svc_entry.assigned);
                }
            }
        }
    }

    Ok(())
}
