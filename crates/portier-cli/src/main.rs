use clap::builder::styling::{AnsiColor, Styles};
use clap::{Parser, Subcommand};

mod commands;
mod output;

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().bold())
    .usage(AnsiColor::Green.on_default().bold())
    .literal(AnsiColor::Cyan.on_default().bold())
    .placeholder(AnsiColor::Cyan.on_default());

const AFTER_HELP: &str = "\x1b[1;32mExamples:\x1b[0m
  \x1b[36mportier scan\x1b[0m                  See what's listening and spot conflicts
  \x1b[36mportier run -- npm run dev\x1b[0m     Run a dev server on a guaranteed-free port
  \x1b[36mportier start\x1b[0m                  Detect, assign free ports, rewrite configs, launch
  \x1b[36mportier link\x1b[0m                   Register the current project
  \x1b[36mportier status\x1b[0m                 List your registered projects

\x1b[1;32mFirst time?\x1b[0m
  Run \x1b[36mportier scan\x1b[0m in any directory — it needs no setup. When you hit a
  conflict, \x1b[36mportier run -- <your dev command>\x1b[0m fixes it automatically.";

#[derive(Parser)]
#[command(
    name = "portier",
    version,
    about = "Never fight a port conflict again.",
    long_about = "Portier detects port conflicts and rewrites your project's config files \
                  (docker-compose, .env, nginx, package.json) so the real source of truth \
                  matches the assigned port — with snapshots for instant rollback.",
    arg_required_else_help = true,
    propagate_version = true,
    styles = STYLES,
    after_help = AFTER_HELP,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List listening ports and highlight conflicts
    Scan(commands::scan::ScanArgs),
    /// Run a command on a guaranteed-free port (auto-injects PORT)
    Run(commands::run::RunArgs),
    /// Detect, assign free ports, rewrite configs, and launch
    Start(commands::start::StartArgs),
    /// Register the current project so Portier can manage it
    Link(commands::link::LinkArgs),
    /// Show your registered projects and their ports
    Status(commands::status::StatusArgs),
    /// Assign free ports to a registered project
    Assign(commands::assign::AssignArgs),
    /// Create or edit a project's portier.json config
    Config(commands::config::ConfigArgs),
    /// View or initialize global settings (~/.config/portier/config.toml)
    Settings(commands::settings::SettingsArgs),
    /// Stop a project's running services
    Stop(commands::stop::StopArgs),
    /// Stop one project and start another
    Switch(commands::switch::SwitchArgs),
    /// Restore config files from a snapshot
    Rollback(commands::rollback::RollbackArgs),
    /// Run the background auto-healer (launchd/systemd)
    Daemon(commands::daemon::DaemonArgs),
    /// Wire up AI agents (writes .mcp.json, Cursor rules, CLAUDE.md)
    InitAgent(commands::init_agent::InitAgentArgs),
    /// Start the MCP server (JSON-RPC over stdio)
    Mcp(commands::mcp::McpArgs),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan(args) => commands::scan::run(args),
        Commands::Status(args) => commands::status::run(args),
        Commands::Assign(args) => commands::assign::run(args),
        Commands::Config(args) => commands::config::run(args),
        Commands::Settings(args) => commands::settings::run(args),
        Commands::Link(args) => commands::link::run(args),
        Commands::Start(args) => commands::start::run(args),
        Commands::Stop(args) => commands::stop::run(args),
        Commands::Switch(args) => commands::switch::run(args),
        Commands::Rollback(args) => commands::rollback::run(args),
        Commands::InitAgent(args) => commands::init_agent::run(args),
        Commands::Run(args) => commands::run::run(args),
        Commands::Daemon(args) => commands::daemon::run(args),
        Commands::Mcp(args) => commands::mcp::run(args),
    }
}
