use crate::error::Result;
use crate::rewriter::{ConfigBackend, DiffLine, PortDeclaration};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct DotenvBackend {
    path: PathBuf,
}

impl DotenvBackend {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }
}

impl ConfigBackend for DotenvBackend {
    fn can_handle(&self) -> bool {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == ".env" || n.starts_with(".env."))
            .unwrap_or(false)
    }

    fn read_ports(&self) -> Result<Vec<PortDeclaration>> {
        let content = fs::read_to_string(&self.path)?;
        let mut ports = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            if let Some(eq_pos) = trimmed.find('=') {
                let var_name = trimmed[..eq_pos].trim().to_string();
                let val = trimmed[eq_pos + 1..].trim();
                if let Ok(port) = val.parse::<u16>() {
                    ports.push(PortDeclaration {
                        service: var_name.clone(),
                        key: var_name,
                        port,
                        file_path: self.path.to_string_lossy().to_string(),
                    });
                }
            }
        }

        Ok(ports)
    }

    fn write_ports(&mut self, assignments: &HashMap<String, u16>) -> Result<()> {
        let content = fs::read_to_string(&self.path)?;
        let has_trailing_newline = content.ends_with('\n');
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            if let Some(eq_pos) = trimmed.find('=') {
                let var_name = trimmed[..eq_pos].trim();
                if let Some(&new_port) = assignments.get(var_name) {
                    let val_part = trimmed[eq_pos + 1..].trim();
                    if val_part.parse::<u16>().is_ok() {
                        // Preserve leading whitespace/formatting
                        let indent = &line[..line.len() - line.trim_start().len()];
                        *line = format!("{}{}={}", indent, var_name, new_port);
                    }
                }
            }
        }

        let output = if has_trailing_newline {
            format!("{}\n", lines.join("\n"))
        } else {
            lines.join("\n")
        };
        fs::write(&self.path, output)?;
        Ok(())
    }

    fn dry_run(&self, assignments: &HashMap<String, u16>) -> Result<Vec<DiffLine>> {
        let content = fs::read_to_string(&self.path)?;
        let mut diffs = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            if let Some(eq_pos) = trimmed.find('=') {
                let var_name = trimmed[..eq_pos].trim();
                if let Some(&new_port) = assignments.get(var_name) {
                    let val = trimmed[eq_pos + 1..].trim();
                    if val.parse::<u16>().is_ok() {
                        diffs.push(DiffLine::Replaced {
                            old: line.to_string(),
                            new: format!("{}={}", var_name, new_port),
                        });
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

    #[test]
    fn test_can_handle_matches_dotenv() {
        let b = DotenvBackend::new(Path::new(".env"));
        assert!(b.can_handle());
    }

    #[test]
    fn test_can_handle_matches_dotenv_production() {
        let b = DotenvBackend::new(Path::new(".env.production"));
        assert!(b.can_handle());
    }

    #[test]
    fn test_can_handle_rejects_myapp_env() {
        let b = DotenvBackend::new(Path::new("myapp.env"));
        assert!(!b.can_handle());
    }

    #[test]
    fn test_can_handle_rejects_env_production() {
        let b = DotenvBackend::new(Path::new("env.production"));
        assert!(!b.can_handle());
    }

    #[test]
    fn test_trailing_newline_preserved() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let content = "PORT=3000\n";
        std::io::Write::write_all(&mut file, content.as_bytes()).unwrap();
        let path = file.path().to_path_buf();

        let mut backend = DotenvBackend::new(&path);
        let mut assignments = HashMap::new();
        assignments.insert("PORT".to_string(), 8080);
        backend.write_ports(&assignments).unwrap();

        let result = fs::read_to_string(&path).unwrap();
        assert!(result.ends_with('\n'), "trailing newline should be preserved");
        assert!(result.contains("PORT=8080"), "port value should be updated");
    }

    #[test]
    fn test_trailing_newline_not_added() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let content = "PORT=3000";
        std::io::Write::write_all(&mut file, content.as_bytes()).unwrap();
        let path = file.path().to_path_buf();

        let mut backend = DotenvBackend::new(&path);
        let mut assignments = HashMap::new();
        assignments.insert("PORT".to_string(), 8080);
        backend.write_ports(&assignments).unwrap();

        let result = fs::read_to_string(&path).unwrap();
        assert!(!result.ends_with('\n'), "no trailing newline should not be added");
        assert!(result.contains("PORT=8080"), "port value should be updated");
    }

    #[test]
    fn test_dotenv_roundtrip_multiple_vars() {
        // ponytail: full read-modify-write-read cycle with multiple env vars
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let content = "PORT=3000\nAPI_PORT=4000\nHOST=localhost\n";
        std::io::Write::write_all(&mut file, content.as_bytes()).unwrap();
        let path = file.path().to_path_buf();

        // Read — should detect only port-like values
        let backend = DotenvBackend::new(&path);
        let ports = backend.read_ports().unwrap();
        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].key, "PORT");
        assert_eq!(ports[0].port, 3000);
        assert_eq!(ports[1].key, "API_PORT");
        assert_eq!(ports[1].port, 4000);

        // Assign new ports
        let mut assignments = HashMap::new();
        for p in &ports {
            assignments.insert(p.key.clone(), p.port + 1000);
        }

        // Write back
        let mut backend = DotenvBackend::new(&path);
        backend.write_ports(&assignments).unwrap();

        // Read back and verify
        let backend = DotenvBackend::new(&path);
        let ports2 = backend.read_ports().unwrap();
        assert_eq!(ports2.len(), 2);
        for p in &ports2 {
            let original = ports.iter().find(|op| op.key == p.key).unwrap();
            assert_eq!(p.port, original.port + 1000);
        }

        // Verify file content: port values updated, other vars untouched, trailing newline preserved
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains("PORT=4000"));
        assert!(result.contains("API_PORT=5000"));
        assert!(result.contains("HOST=localhost"));
        assert!(result.ends_with('\n'), "trailing newline should be preserved");
    }

    #[test]
    fn test_can_handle_dotenv_local() {
        let b = DotenvBackend::new(Path::new(".env.local"));
        assert!(b.can_handle());
    }

    #[test]
    fn test_can_handle_dotenv_staging() {
        let b = DotenvBackend::new(Path::new(".env.staging"));
        assert!(b.can_handle());
    }
}
