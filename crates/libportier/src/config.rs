use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    pub preferred: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub version: u32,
    pub name: String,
    pub stack: String,
    #[serde(default)]
    pub ports: HashMap<String, PortConfig>,
    #[serde(default)]
    pub services: HashMap<String, ServiceConfig>,
    #[serde(default)]
    pub linked: bool,
}

impl ProjectConfig {
    pub fn new(name: String, stack: String) -> Self {
        Self {
            schema: None,
            version: 1,
            name,
            stack,
            ports: HashMap::new(),
            services: HashMap::new(),
            linked: false,
        }
    }

    pub fn load(root: &Path) -> Result<Option<Self>> {
        let path = root.join("portier.json");
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&data)?))
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let path = root.join("portier.json");
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    pub fn assigned_ports(&self) -> HashMap<String, u16> {
        let mut result = HashMap::new();
        for (name, config) in &self.ports {
            result.insert(name.clone(), config.assigned.unwrap_or(config.preferred));
        }
        result
    }
}

pub fn find_config_files(root: &Path, stack: &str) -> Vec<String> {
    let mut files = Vec::new();
    for pattern in config_files_for_stack(stack) {
        let path = root.join(pattern);
        if path.exists() {
            files.push(pattern.to_string());
        }
    }
    files
}

fn config_files_for_stack(stack: &str) -> &[&str] {
    match stack {
        "Node" => &["package.json", ".env"],
        "Python" => &[".env", "docker-compose.yml"],
        "Ruby" => &[".env", "database.yml"],
        "Rust" => &[".env"],
        "Go" => &[".env"],
        "Docker" => &["docker-compose.yml", ".env"],
        _ => &[".env"],
    }
}

pub fn extract_env_port(root: &Path) -> Result<HashMap<String, u16>> {
    let env_path = root.join(".env");
    if !env_path.exists() {
        return Ok(HashMap::new());
    }
    let content = fs::read_to_string(&env_path)?;
    let mut ports = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            continue;
        }
        let key = parts[0].trim().to_string();
        let val = parts[1].trim().trim_matches('"').to_string();
        if let Ok(port) = val.parse::<u16>() {
            ports.insert(key, port);
        }
    }
    Ok(ports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_config_new() {
        let c = ProjectConfig::new("my-app".into(), "Node".into());
        assert_eq!(c.name, "my-app");
        assert_eq!(c.stack, "Node");
        assert!(c.ports.is_empty());
    }

    #[test]
    fn test_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = ProjectConfig::new("roundtrip".into(), "Rust".into());
        c.ports.insert(
            "web".into(),
            PortConfig {
                preferred: 3000,
                assigned: Some(3001),
            },
        );
        c.save(dir.path()).unwrap();

        let loaded = ProjectConfig::load(dir.path()).unwrap().unwrap();
        assert_eq!(loaded.name, "roundtrip");
        assert_eq!(loaded.ports["web"].preferred, 3000);
        assert_eq!(loaded.ports["web"].assigned, Some(3001));
    }

    #[test]
    fn test_config_load_missing() {
        let dir = tempfile::tempdir().unwrap();
        let result = ProjectConfig::load(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_assigned_ports() {
        let mut c = ProjectConfig::new("test".into(), "Node".into());
        c.ports.insert(
            "web".into(),
            PortConfig {
                preferred: 3000,
                assigned: None,
            },
        );
        c.ports.insert(
            "api".into(),
            PortConfig {
                preferred: 4000,
                assigned: Some(4001),
            },
        );

        let assigned = c.assigned_ports();
        assert_eq!(assigned["web"], 3000); // falls back to preferred
        assert_eq!(assigned["api"], 4001);
    }

    #[test]
    fn test_extract_env_port() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".env"),
            "PORT=3000\nAPI_PORT=4001\n# COMMENT=9999\nFOO=bar\n",
        )
        .unwrap();

        let ports = extract_env_port(dir.path()).unwrap();
        assert_eq!(ports.get("PORT"), Some(&3000));
        assert_eq!(ports.get("API_PORT"), Some(&4001));
        assert!(ports.get("COMMENT").is_none());
        assert!(ports.get("FOO").is_none());
    }

    #[test]
    fn test_extract_env_port_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let ports = extract_env_port(dir.path()).unwrap();
        assert!(ports.is_empty());
    }

    #[test]
    fn test_extract_env_port_quoted_values() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".env"), "PORT=\"3000\"\n").unwrap();
        let ports = extract_env_port(dir.path()).unwrap();
        assert_eq!(ports.get("PORT"), Some(&3000));
    }

    #[test]
    fn test_config_files_for_stack() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join(".env"), "").unwrap();

        let files = find_config_files(dir.path(), "Node");
        assert!(files.contains(&"package.json".to_string()));
        assert!(files.contains(&".env".to_string()));
    }
}
