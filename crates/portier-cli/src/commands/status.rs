use clap::Args;

use crate::output;

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Project name (optional, shows all if omitted)
    pub name: Option<String>,
    /// JSON output
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: StatusArgs) -> anyhow::Result<()> {
    let registry = libportier::registry::Registry::load()?;

    if args.json {
        output::print_json(&registry);
        return Ok(());
    }

    if registry.projects.is_empty() {
        output::print_warning("No projects registered yet.");
        output::print_hint("Register the current project with: portier link");
        return Ok(());
    }

    let projects: Vec<_> = if let Some(ref name) = args.name {
        registry
            .projects
            .iter()
            .filter(|(_, e)| e.name == *name)
            .collect()
    } else {
        registry.projects.iter().collect()
    };

    let headers = &["Name", "Stack", "Path", "Services"];
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (path, entry) in &projects {
        let services: String = entry
            .services
            .iter()
            .map(|(s, svc)| format!("{}:{}", s, svc.assigned))
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(vec![
            entry.name.clone(),
            entry.stack.clone(),
            output::home_relative(path),
            if services.is_empty() {
                "—".to_string()
            } else {
                services
            },
        ]);
    }
    output::print_table(headers, &rows);

    Ok(())
}
