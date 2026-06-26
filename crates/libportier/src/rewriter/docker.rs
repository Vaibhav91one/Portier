use crate::error::Result;
use crate::rewriter::{ConfigBackend, DiffLine, PortDeclaration};
use serde_yaml;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct DockerComposeBackend {
    path: PathBuf,
}

impl DockerComposeBackend {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }
}

impl ConfigBackend for DockerComposeBackend {
    fn can_handle(&self) -> bool {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                n == "docker-compose.yml"
                    || n == "docker-compose.yaml"
                    || n == "compose.yml"
                    || n == "compose.yaml"
            })
            .unwrap_or(false)
    }

    fn read_ports(&self) -> Result<Vec<PortDeclaration>> {
        let content = fs::read_to_string(&self.path)?;
        let value: serde_yaml::Value = serde_yaml::from_str(&content)?;
        let mut ports = Vec::new();

        if let Some(services) = value.get("services").and_then(|s| s.as_mapping()) {
            for (service_name, service_config) in services {
                let service_name_str = service_name.as_str().unwrap_or("unknown");
                if let Some(ports_list) = service_config.get("ports").and_then(|p| p.as_sequence()) {
                    for (i, port_entry) in ports_list.iter().enumerate() {
                        if let Some(port_str) = port_entry.as_str() {
                            let parts: Vec<&str> = port_str.split(':').collect();
                            // ponytail: second-to-last segment handles ip:host:container and host:container
                            if parts.len() >= 2 {
                                if let Ok(port) = parts[parts.len() - 2].parse::<u16>() {
                                    ports.push(PortDeclaration {
                                        service: service_name_str.to_string(),
                                        key: format!("{}:ports[{}]", service_name_str, i),
                                        port,
                                        file_path: self.path.to_string_lossy().to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(ports)
    }

    fn write_ports(&mut self, assignments: &HashMap<String, u16>) -> Result<()> {
        let content = fs::read_to_string(&self.path)?;
        let mut value: serde_yaml::Value = serde_yaml::from_str(&content)?;

        if let Some(services) = value.get_mut("services").and_then(|s| s.as_mapping_mut()) {
            for (service_name, service_config) in services.iter_mut() {
                let service_name_str = service_name.as_str().unwrap_or("unknown");
                if let Some(ports_list) = service_config.get_mut("ports").and_then(|p| p.as_sequence_mut()) {
                    for (i, port_entry) in ports_list.iter_mut().enumerate() {
                        let key = format!("{}:ports[{}]", service_name_str, i);
                        if let Some(&new_port) = assignments.get(&key) {
                            if let Some(port_str) = port_entry.as_str() {
                                if let Some(colon_pos) = port_str.rfind(':') {
                                    let container_part = &port_str[colon_pos + 1..];
                                    *port_entry = serde_yaml::Value::String(format!("{}:{}", new_port, container_part));
                                } else {
                                    // Single port value, replace entirely
                                    *port_entry = serde_yaml::Value::String(new_port.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        let new_content = serde_yaml::to_string(&value)?;
        fs::write(&self.path, &new_content)?;
        Ok(())
    }

    fn dry_run(&self, assignments: &HashMap<String, u16>) -> Result<Vec<DiffLine>> {
        let content = fs::read_to_string(&self.path)?;
        let value: serde_yaml::Value = serde_yaml::from_str(&content)?;
        let mut diffs = Vec::new();

        if let Some(services) = value.get("services").and_then(|s| s.as_mapping()) {
            for (service_name, service_config) in services {
                let service_name_str = service_name.as_str().unwrap_or("unknown");
                if let Some(ports_list) = service_config.get("ports").and_then(|p| p.as_sequence()) {
                    for (i, port_entry) in ports_list.iter().enumerate() {
                        let key = format!("{}:ports[{}]", service_name_str, i);
                        if let Some(&new_port) = assignments.get(&key) {
                            if let Some(port_str) = port_entry.as_str() {
                                if let Some(colon_pos) = port_str.rfind(':') {
                                    let container_part = &port_str[colon_pos + 1..];
                                    let new_entry = format!("{}:{}", new_port, container_part);
                                    diffs.push(DiffLine::Replaced {
                                        old: port_str.to_string(),
                                        new: new_entry,
                                    });
                                } else {
                                    diffs.push(DiffLine::Replaced {
                                        old: port_str.to_string(),
                                        new: new_port.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(diffs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_compose(file: &mut tempfile::NamedTempFile, yaml: &str) -> PathBuf {
        write!(file, "{}", yaml).unwrap();
        file.path().to_path_buf()
    }

    #[test]
    fn test_read_ports_host_container() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let path = write_compose(&mut file, r#"
services:
  web:
    ports:
      - "3000:3000"
"#);
        let backend = DockerComposeBackend::new(&path);
        let ports = backend.read_ports().unwrap();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 3000);
        assert_eq!(ports[0].service, "web");
    }

    #[test]
    fn test_read_ports_ip_host_container() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let path = write_compose(&mut file, r#"
services:
  web:
    ports:
      - "127.0.0.1:3000:3000"
"#);
        let backend = DockerComposeBackend::new(&path);
        let ports = backend.read_ports().unwrap();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 3000);
        assert_eq!(ports[0].service, "web");
    }

    #[test]
    fn test_read_ports_mapped_ports() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let path = write_compose(&mut file, r#"
services:
  web:
    ports:
      - "8080:80"
"#);
        let backend = DockerComposeBackend::new(&path);
        let ports = backend.read_ports().unwrap();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 8080);
        assert_eq!(ports[0].service, "web");
    }

    #[test]
    fn test_read_ports_roundtrip_write() {
        // Write original, read ports, assign new port, write back, read again
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let path = write_compose(&mut file, r#"
services:
  web:
    ports:
      - "127.0.0.1:3000:3000"
"#);
        let backend = DockerComposeBackend::new(&path);
        let ports = backend.read_ports().unwrap();
        assert_eq!(ports[0].port, 3000);

        let mut assignments = HashMap::new();
        assignments.insert(ports[0].key.clone(), 4000u16);
        let mut backend = DockerComposeBackend::new(&path);
        backend.write_ports(&assignments).unwrap();

        let backend = DockerComposeBackend::new(&path);
        let ports = backend.read_ports().unwrap();
        assert_eq!(ports[0].port, 4000);
    }

    #[test]
    fn test_docker_compose_integration() {
        // ponytail: integration test with temp dir, multiple services, mixed formats
        let tmp = tempfile::tempdir().unwrap();
        let compose_path = tmp.path().join("docker-compose.yml");

        let yaml = r#"
services:
  web:
    ports:
      - "3000:3000"
      - "127.0.0.1:3001:3001"
  api:
    ports:
      - "4000:4000"
"#;
        std::fs::write(&compose_path, yaml).unwrap();

        // Read ports — should find 3 declarations across two services
        let backend = DockerComposeBackend::new(&compose_path);
        let ports = backend.read_ports().unwrap();
        assert_eq!(ports.len(), 3);
        assert_eq!(ports.iter().filter(|p| p.service == "web").count(), 2);
        assert_eq!(ports.iter().filter(|p| p.service == "api").count(), 1);

        // Assign new ports (+1000 to each)
        let mut assignments = HashMap::new();
        for p in &ports {
            assignments.insert(p.key.clone(), p.port + 1000);
        }

        // Write back
        let mut backend = DockerComposeBackend::new(&compose_path);
        backend.write_ports(&assignments).unwrap();

        // Read again and verify every port was bumped
        let backend = DockerComposeBackend::new(&compose_path);
        let ports2 = backend.read_ports().unwrap();
        assert_eq!(ports2.len(), 3);
        for p in &ports2 {
            let original = ports.iter().find(|op| op.key == p.key).unwrap();
            assert_eq!(p.port, original.port + 1000);
        }

        // Verify file content directly shows the new port assignments
        let content = std::fs::read_to_string(&compose_path).unwrap();
        assert!(content.contains("4000:3000"));
        assert!(content.contains("4001:3001"));
        assert!(content.contains("5000:4000"));
    }
}
