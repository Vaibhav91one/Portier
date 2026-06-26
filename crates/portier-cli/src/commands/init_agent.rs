use clap::Args;

use crate::output;

#[derive(Args, Debug)]
pub struct InitAgentArgs {
    /// Target project directory
    #[arg(short, long, default_value = ".")]
    pub dir: String,
    /// Skip all confirmation prompts
    #[arg(short, long)]
    pub yes: bool,
}

pub fn run(args: InitAgentArgs) -> anyhow::Result<()> {
    let target = std::path::Path::new(&args.dir);
    let target_path = target.canonicalize()?;

    // 1. Create .mcp.json
    let mcp_path = target_path.join(".mcp.json");
    let portier_bin = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "portier".to_string());
    let mcp_config = serde_json::json!({
        "mcpServers": {
            "portier": {
                "command": portier_bin,
                "args": ["mcp"]
            }
        }
    });
    std::fs::write(&mcp_path, serde_json::to_string_pretty(&mcp_config)?)?;
    output::print_success(format!("Created {}", mcp_path.display()));

    // 2. Create .cursor/rules/portier.mdc
    let cursor_rules_dir = target_path.join(".cursor/rules");
    std::fs::create_dir_all(&cursor_rules_dir)?;
    let cursor_rule = r#"---
description: >
  Auto-detect and fix port conflicts. Always use portier before starting
  dev servers, when hitting EADDRINUSE, or when managing multiple projects.
globs: "**/*"
alwaysApply: true
---

## Portier Port Management

Before starting ANY dev server, run `portier scan --json` for port conflicts.
If port 3000/5173/8080 is in use, run `portier assign . --range 3000-4000`.
Use `portier run -- <command>` instead of running commands directly.
"#;
    let cursor_rule_path = cursor_rules_dir.join("portier.mdc");
    std::fs::write(&cursor_rule_path, cursor_rule)?;
    output::print_success(format!("Created {}", cursor_rule_path.display()));

    // 3. Add to CLAUDE.md
    let claude_md_path = target_path.join("CLAUDE.md");
    if claude_md_path.exists() && (args.yes || confirm("Add Portier section to CLAUDE.md?")) {
        let mut content = std::fs::read_to_string(&claude_md_path)?;
        if !content.contains("Portier") {
            content.push_str("\n\n## Port Management\n\n");
            content.push_str("Portier handles port conflicts automatically:\n");
            content.push_str("- `portier scan` before starting any dev server\n");
            content.push_str("- `portier start` instead of `npm run dev`\n");
            content.push_str("- `portier run -- <cmd>` to prevent EADDRINUSE\n");
            content.push_str("- `portier daemon` for background auto-healing\n");
            std::fs::write(&claude_md_path, &content)?;
            output::print_success("Updated CLAUDE.md");
        } else {
            output::print_warning("Portier already in CLAUDE.md, skipping.");
        }
    }

    output::print_success("Portier agent integration complete!");
    output::print_success("Restart Claude Code/Cursor to activate MCP server.");
    Ok(())
}

fn confirm(prompt: &str) -> bool {
    print!("{} [Y/n] ", prompt);
    std::io::Write::flush(&mut std::io::stdout()).ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let input = input.trim().to_lowercase();
    input.is_empty() || input == "y" || input == "yes"
}
