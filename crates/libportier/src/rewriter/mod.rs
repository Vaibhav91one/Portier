use crate::error::Result;
use crate::snapshot::{Snapshot, take_snapshot};
use std::collections::HashMap;
use std::path::Path;

pub mod docker;
pub mod dotenv;
pub mod nginx;
pub mod generic;

#[derive(Debug, Clone)]
pub struct PortDeclaration {
    pub service: String,
    pub key: String,
    pub port: u16,
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLine {
    Unchanged(String),
    Removed(String),
    Added(String),
    Replaced { old: String, new: String },
}

pub trait ConfigBackend {
    fn can_handle(&self) -> bool;
    fn read_ports(&self) -> Result<Vec<PortDeclaration>>;
    fn write_ports(&mut self, assignments: &HashMap<String, u16>) -> Result<()>;
    fn dry_run(&self, assignments: &HashMap<String, u16>) -> Result<Vec<DiffLine>>;
}

pub fn from_path(path: &Path) -> Option<Box<dyn ConfigBackend>> {
    let candidates: [Box<dyn ConfigBackend>; 4] = [
        Box::new(docker::DockerComposeBackend::new(path)),
        Box::new(dotenv::DotenvBackend::new(path)),
        Box::new(nginx::NginxBackend::new(path)),
        Box::new(generic::GenericBackend::new(path)),
    ];
    // ponytail: constructs all backends then picks — cheap for CLI init
    candidates.into_iter().find(|b| b.can_handle())
}

/// Apply port assignments to config files on disk.
/// Takes a snapshot before writing and returns it.
pub fn apply_assignments(
    assignments: &HashMap<String, u16>,
    config_files: &[String],
    project_root: &Path,
) -> Result<Snapshot> {
    let snapshot = take_snapshot(project_root, config_files)?;

    for config_file in config_files {
        let full_path = project_root.join(config_file);
        if !full_path.exists() {
            continue;
        }
        if let Some(mut backend) = from_path(&full_path) {
            let declarations = backend.read_ports()?;
            let mut key_assignments: HashMap<String, u16> = HashMap::new();
            for decl in &declarations {
                if let Some(&port) = assignments.get(&decl.service) {
                    key_assignments.insert(decl.key.clone(), port);
                }
            }
            if !key_assignments.is_empty() {
                backend.write_ports(&key_assignments)?;
            }
        }
    }

    Ok(snapshot)
}

/// Preview port assignments without modifying files.
/// Returns per-file diff lists.
pub fn preview_assignments(
    assignments: &HashMap<String, u16>,
    config_files: &[String],
    project_root: &Path,
) -> Result<Vec<(String, Vec<DiffLine>)>> {
    let mut results = Vec::new();

    for config_file in config_files {
        let full_path = project_root.join(config_file);
        if !full_path.exists() {
            continue;
        }
        if let Some(backend) = from_path(&full_path) {
            let declarations = backend.read_ports()?;
            let mut key_assignments: HashMap<String, u16> = HashMap::new();
            for decl in &declarations {
                if let Some(&port) = assignments.get(&decl.service) {
                    key_assignments.insert(decl.key.clone(), port);
                }
            }
            if !key_assignments.is_empty() {
                let diffs = backend.dry_run(&key_assignments)?;
                results.push((config_file.clone(), diffs));
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_assignments_updates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Create docker-compose.yml with two services
        let compose_path = root.join("docker-compose.yml");
        std::fs::write(
            &compose_path,
            "services:\n  web:\n    ports:\n      - \"3000:3000\"\n  api:\n    ports:\n      - \"4000:4000\"\n",
        )
        .unwrap();

        // Create .env
        let env_path = root.join(".env");
        std::fs::write(&env_path, "PORT=3000\nAPI_PORT=4000\n").unwrap();

        let config_files = vec!["docker-compose.yml".to_string(), ".env".to_string()];

        let mut assignments = HashMap::new();
        assignments.insert("web".to_string(), 5000u16);
        assignments.insert("PORT".to_string(), 5000u16);

        let snapshot = apply_assignments(&assignments, &config_files, root).unwrap();
        assert!(!snapshot.id.is_empty());

        // Verify compose was updated
        let compose_content = std::fs::read_to_string(&compose_path).unwrap();
        assert!(compose_content.contains("5000:3000"));

        // Verify .env was updated
        let env_content = std::fs::read_to_string(&env_path).unwrap();
        assert!(env_content.contains("PORT=5000"));
    }

    #[test]
    fn test_preview_assignments_no_modify() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let compose_path = root.join("docker-compose.yml");
        std::fs::write(
            &compose_path,
            "services:\n  web:\n    ports:\n      - \"3000:3000\"\n",
        )
        .unwrap();

        let config_files = vec!["docker-compose.yml".to_string()];

        let mut assignments = HashMap::new();
        assignments.insert("web".to_string(), 5000u16);

        let diffs = preview_assignments(&assignments, &config_files, root).unwrap();
        assert_eq!(diffs.len(), 1);
        let (file, file_diffs) = &diffs[0];
        assert!(file.contains("docker-compose.yml"));
        assert!(!file_diffs.is_empty());

        // Verify file was NOT modified
        let content = std::fs::read_to_string(&compose_path).unwrap();
        assert!(content.contains("3000:3000"));
    }
}
