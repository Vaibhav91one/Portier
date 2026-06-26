use clap::Args;
use std::path::PathBuf;

use crate::output;

#[derive(Args, Debug)]
pub struct RollbackArgs {
    /// Snapshot ID to restore (list available if omitted)
    pub snapshot_id: Option<String>,
}

/// Compute snapshots directory from the project's worktree if registered,
/// falling back to cwd directory name.
// ponytail: single-pass registry lookup avoids loading it multiple times.
fn snapshots_dir() -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let cwd_str = cwd.to_string_lossy();

    // ponytail: prefer registered worktree so snapshots follow the project,
    //           not the shell's cwd.
    let project_root = libportier::registry::Registry::load()
        .ok()
        .and_then(|reg| {
            let found = reg.projects.iter().find(|(path, e)| {
                e.worktree.as_deref() == Some(cwd_str.as_ref()) || path.as_str() == cwd_str.as_ref()
            });
            found.and_then(|(_, e)| e.worktree.clone())
        })
        .unwrap_or_else(|| cwd_str.to_string());

    let dir_name = PathBuf::from(&project_root)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".into());
    // ponytail: stdlib HOME instead of adding dirs_next dependency
    let home = std::env::var("HOME")
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory: HOME not set"))?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("portier")
        .join("snapshots")
        .join(dir_name))
}

pub fn run(args: RollbackArgs) -> anyhow::Result<()> {
    let sdir = snapshots_dir()?;

    match args.snapshot_id {
        None => {
            // List available snapshots
            let ids = libportier::get_snapshots(&sdir)?;
            if ids.is_empty() {
                output::print_warning("No snapshots found for current project.");
                return Ok(());
            }
            output::print_success("Snapshots:");
            for id in &ids {
                let snap = libportier::load_snapshot(id, &sdir)?;
                println!("    {}  {} files  {}", id, snap.files.len(), snap.timestamp);
            }
        }
        Some(id) => {
            // Restore files from snapshot
            let snap = libportier::load_snapshot(&id, &sdir)?;
            let count = snap.files.len();
            libportier::restore_snapshot(&snap)?;
            output::print_success(format!("Restored {} files from snapshot {}.", count, id));
        }
    }

    Ok(())
}
