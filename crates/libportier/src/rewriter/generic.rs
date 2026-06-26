use crate::error::{PortierError, Result};
use crate::rewriter::{ConfigBackend, DiffLine, PortDeclaration};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct GenericBackend {
    path: PathBuf,
}

impl GenericBackend {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    fn is_json(&self) -> bool {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "json")
            .unwrap_or(false)
    }

    fn is_yaml(&self) -> bool {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false)
    }

    // JSON helpers -----------------------------------------------------------

    fn collect_json_ports(value: &serde_json::Value, path: &str, file_path: &str) -> Vec<PortDeclaration> {
        match value {
            serde_json::Value::Number(n) => {
                if path.to_ascii_lowercase().contains("port") {
                    if let Some(port) = n.as_u64().and_then(|n| u16::try_from(n).ok()) {
                        return vec![PortDeclaration {
                            service: "generic".to_string(),
                            key: if path.is_empty() { "$".to_string() } else { path.to_string() },
                            port,
                            file_path: file_path.to_string(),
                        }];
                    }
                }
                vec![]
            }
            serde_json::Value::Object(map) => {
                let mut ports = vec![];
                for (k, v) in map {
                    let child = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{}/{}", path, k)
                    };
                    ports.extend(Self::collect_json_ports(v, &child, file_path));
                }
                ports
            }
            serde_json::Value::Array(arr) => {
                let mut ports = vec![];
                for (i, v) in arr.iter().enumerate() {
                    let child = format!("{}/{}", path, i);
                    ports.extend(Self::collect_json_ports(v, &child, file_path));
                }
                ports
            }
            _ => vec![],
        }
    }

    fn set_json_at_path(value: &mut serde_json::Value, path: &str, new_port: u16) -> Result<()> {
        if path == "$" {
            *value = serde_json::json!(new_port);
            return Ok(());
        }
        let (first, rest) = path.split_once('/').unwrap_or((path, ""));
        let rest_is_empty = rest.is_empty();
        if let Ok(i) = first.parse::<usize>() {
            if rest_is_empty {
                value[i] = serde_json::json!(new_port);
            } else {
                Self::set_json_at_path(
                    value
                        .get_mut(i)
                        .ok_or_else(|| PortierError::Config(format!("path not found: {}", path)))?,
                    rest,
                    new_port,
                )?;
            }
        } else {
            if rest_is_empty {
                value[first] = serde_json::json!(new_port);
            } else {
                Self::set_json_at_path(
                    value
                        .get_mut(first)
                        .ok_or_else(|| PortierError::Config(format!("path not found: {}", path)))?,
                    rest,
                    new_port,
                )?;
            }
        }
        Ok(())
    }

    // YAML helpers -----------------------------------------------------------

    fn collect_yaml_ports(value: &serde_yaml::Value, path: &str, file_path: &str) -> Vec<PortDeclaration> {
        match value {
            serde_yaml::Value::Number(n) => {
                if path.to_ascii_lowercase().contains("port") {
                    if let Some(port) = n.as_u64().and_then(|n| u16::try_from(n).ok()) {
                        return vec![PortDeclaration {
                            service: "generic".to_string(),
                            key: if path.is_empty() { "$".to_string() } else { path.to_string() },
                            port,
                            file_path: file_path.to_string(),
                        }];
                    }
                }
                vec![]
            }
            serde_yaml::Value::Mapping(map) => {
                let mut ports = vec![];
                for (k, v) in map {
                    let k_str = k.as_str().map(|s| s.to_string()).unwrap_or_default();
                    let child = if path.is_empty() {
                        k_str.clone()
                    } else {
                        format!("{}/{}", path, k_str)
                    };
                    ports.extend(Self::collect_yaml_ports(v, &child, file_path));
                }
                ports
            }
            serde_yaml::Value::Sequence(seq) => {
                let mut ports = vec![];
                for (i, v) in seq.iter().enumerate() {
                    let child = format!("{}/{}", path, i);
                    ports.extend(Self::collect_yaml_ports(v, &child, file_path));
                }
                ports
            }
            _ => vec![],
        }
    }

    fn set_yaml_at_path(value: &mut serde_yaml::Value, path: &str, new_port: u16) -> Result<()> {
        if path == "$" {
            *value = serde_yaml::Value::Number(serde_yaml::Number::from(new_port));
            return Ok(());
        }
        let (first, rest) = path.split_once('/').unwrap_or((path, ""));
        let rest_is_empty = rest.is_empty();
        if let Ok(i) = first.parse::<usize>() {
            let seq = value
                .as_sequence_mut()
                .ok_or_else(|| PortierError::Config(format!("expected sequence at {}", path)))?;
            if rest_is_empty {
                seq[i] = serde_yaml::Value::Number(serde_yaml::Number::from(new_port));
            } else {
                Self::set_yaml_at_path(
                    seq.get_mut(i)
                        .ok_or_else(|| PortierError::Config(format!("index out of bounds: {}", path)))?,
                    rest,
                    new_port,
                )?;
            }
        } else {
            let map = value
                .as_mapping_mut()
                .ok_or_else(|| PortierError::Config(format!("expected mapping at {}", path)))?;
            if rest_is_empty {
                map.insert(
                    serde_yaml::Value::String(first.to_string()),
                    serde_yaml::Value::Number(serde_yaml::Number::from(new_port)),
                );
            } else {
                let next = map
                    .get_mut(&serde_yaml::Value::String(first.to_string()))
                    .ok_or_else(|| PortierError::Config(format!("key not found: {}", path)))?;
                Self::set_yaml_at_path(next, rest, new_port)?;
            }
        }
        Ok(())
    }
}

impl ConfigBackend for GenericBackend {
    fn can_handle(&self) -> bool {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "json" || e == "yaml" || e == "yml")
            .unwrap_or(false)
    }

    fn read_ports(&self) -> Result<Vec<PortDeclaration>> {
        let content = fs::read_to_string(&self.path)?;
        let fp = self.path.to_string_lossy().to_string();

        if self.is_json() {
            let value: serde_json::Value = serde_json::from_str(&content)?;
            Ok(Self::collect_json_ports(&value, "", &fp))
        } else if self.is_yaml() {
            let value: serde_yaml::Value = serde_yaml::from_str(&content)?;
            Ok(Self::collect_yaml_ports(&value, "", &fp))
        } else {
            Ok(Vec::new())
        }
    }

    fn write_ports(&mut self, assignments: &HashMap<String, u16>) -> Result<()> {
        let content = fs::read_to_string(&self.path)?;

        if self.is_json() {
            let mut value: serde_json::Value = serde_json::from_str(&content)?;
            for (path, &new_port) in assignments {
                Self::set_json_at_path(&mut value, path, new_port)?;
            }
            let new_content = serde_json::to_string_pretty(&value)?;
            fs::write(&self.path, &new_content)?;
        } else if self.is_yaml() {
            let mut value: serde_yaml::Value = serde_yaml::from_str(&content)?;
            for (path, &new_port) in assignments {
                Self::set_yaml_at_path(&mut value, path, new_port)?;
            }
            let new_content = serde_yaml::to_string(&value)?;
            fs::write(&self.path, &new_content)?;
        }

        Ok(())
    }

    fn dry_run(&self, assignments: &HashMap<String, u16>) -> Result<Vec<DiffLine>> {
        let ports = self.read_ports()?;
        let mut diffs = Vec::new();

        for decl in &ports {
            if let Some(&new_port) = assignments.get(&decl.key) {
                diffs.push(DiffLine::Replaced {
                    old: decl.port.to_string(),
                    new: new_port.to_string(),
                });
            }
        }

        Ok(diffs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_json_ports() {
        // ponytail: only keys containing "port" (case-insensitive) should be detected
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        let json = r#"{
    "port": 3000,
    "name": "my-service",
    "api_port": 4000,
    "description": "some text"
}"#;
        std::fs::write(&path, json).unwrap();

        let backend = GenericBackend::new(&path);
        let ports = backend.read_ports().unwrap();
        assert_eq!(ports.len(), 2, "only port and api_port keys should be detected");

        let keys: Vec<&str> = ports.iter().map(|p| p.key.as_str()).collect();
        assert!(keys.contains(&"port"));
        assert!(keys.contains(&"api_port"));

        // Assign new ports and verify roundtrip
        let mut assignments = HashMap::new();
        for p in &ports {
            assignments.insert(p.key.clone(), p.port + 1000);
        }

        let mut backend = GenericBackend::new(&path);
        backend.write_ports(&assignments).unwrap();

        let backend = GenericBackend::new(&path);
        let ports2 = backend.read_ports().unwrap();
        assert_eq!(ports2.len(), 2);
        for p in &ports2 {
            let original = ports.iter().find(|op| op.key == p.key).unwrap();
            assert_eq!(p.port, original.port + 1000);
        }
    }

    #[test]
    fn test_generic_yaml_ports() {
        // ponytail: same logic as JSON test but with YAML
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.yaml");
        let yaml = r#"
port: 3000
name: my-service
api_port: 4000
description: some text
"#;
        std::fs::write(&path, yaml).unwrap();

        let backend = GenericBackend::new(&path);
        let ports = backend.read_ports().unwrap();
        assert_eq!(ports.len(), 2, "only port and api_port keys should be detected");

        let keys: Vec<&str> = ports.iter().map(|p| p.key.as_str()).collect();
        assert!(keys.contains(&"port"));
        assert!(keys.contains(&"api_port"));

        // Verify non-port values are not detected as port keys
        let names: Vec<&str> = ports.iter().map(|p| p.key.as_str()).collect();
        assert!(!names.contains(&"name"));
        assert!(!names.contains(&"description"));

        // Assign new ports and verify roundtrip
        let mut assignments = HashMap::new();
        for p in &ports {
            assignments.insert(p.key.clone(), p.port + 1000);
        }

        let mut backend = GenericBackend::new(&path);
        backend.write_ports(&assignments).unwrap();

        let backend = GenericBackend::new(&path);
        let ports2 = backend.read_ports().unwrap();
        assert_eq!(ports2.len(), 2);
        for p in &ports2 {
            let original = ports.iter().find(|op| op.key == p.key).unwrap();
            assert_eq!(p.port, original.port + 1000);
        }
    }
}
