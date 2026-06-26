use clap::Args;

#[derive(Args, Debug)]
pub struct McpArgs {}

pub fn run(_args: McpArgs) -> anyhow::Result<()> {
    portier_mcp::run_mcp();
    Ok(())
}
