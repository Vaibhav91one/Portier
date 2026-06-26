use crate::error::Result;
use crate::rewriter::{ConfigBackend, DiffLine, PortDeclaration};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct NginxBackend {
    path: PathBuf,
}

impl NginxBackend {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    /// Extract the port number from a listen directive line.
    /// Handles bare ports: "listen 80;", "listen 80 default_server;"
    /// Handles IPv6: "listen [::1]:80;", "listen [::1]:80 default_server;"
    fn extract_port(line: &str) -> Option<u16> {
        let trimmed = line.trim();
        if !trimmed.starts_with("listen ") || !trimmed.ends_with(';') {
            return None;
        }
        let inner = trimmed[7..trimmed.len() - 1].trim();
        // For IPv6 addresses like [::1]:80, find the last colon and parse
        // everything after it. For bare ports like "80", split whitespace directly.
        let port_str = if let Some(last_colon) = inner.rfind(':') {
            inner[last_colon + 1..].split_whitespace().next()?
        } else {
            inner.split_whitespace().next()?
        };
        port_str.parse::<u16>().ok()
    }

    /// Replace the port number in a listen directive line.
    /// Preserves IPv6 address brackets and any trailing modifiers (e.g. default_server).
    fn replace_port(line: &str, new_port: u16) -> String {
        let trimmed = line.trim();
        let inner = &trimmed[7..trimmed.len() - 1];
        let indent = &line[..line.len() - line.trim_start().len()];

        if let Some(last_colon) = inner.rfind(':') {
            // ponytail: rfind finds the colon before the port in [::1]:80, not one inside brackets
            let before_colon_incl = &inner[..=last_colon]; // includes ':'
            let after_colon = &inner[last_colon + 1..];
            let rest = after_colon
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>()
                .join(" ");
            if rest.is_empty() {
                format!("{}listen {}{};", indent, before_colon_incl, new_port)
            } else {
                format!(
                    "{}listen {}{} {};",
                    indent, before_colon_incl, new_port, rest
                )
            }
        } else {
            let first_token = inner.split_whitespace().next().unwrap_or("");
            let rest = &inner[first_token.len()..];
            format!("{}listen {}{};", indent, new_port, rest)
        }
    }
}

impl ConfigBackend for NginxBackend {
    fn can_handle(&self) -> bool {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "nginx.conf" || n.contains("nginx"))
            .unwrap_or(false)
    }

    fn read_ports(&self) -> Result<Vec<PortDeclaration>> {
        let content = fs::read_to_string(&self.path)?;
        let mut ports = Vec::new();

        for (i, line) in content.lines().enumerate() {
            if let Some(port) = Self::extract_port(line) {
                ports.push(PortDeclaration {
                    service: "nginx".to_string(),
                    key: format!("line[{}]", i),
                    port,
                    file_path: self.path.to_string_lossy().to_string(),
                });
            }
        }

        Ok(ports)
    }

    fn write_ports(&mut self, assignments: &HashMap<String, u16>) -> Result<()> {
        let content = fs::read_to_string(&self.path)?;
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

        for (i, line) in lines.iter_mut().enumerate() {
            let key = format!("line[{}]", i);
            if let Some(&new_port) = assignments.get(&key) {
                if Self::extract_port(line).is_some() {
                    *line = Self::replace_port(line, new_port);
                }
            }
        }

        fs::write(&self.path, lines.join("\n"))?;
        Ok(())
    }

    fn dry_run(&self, assignments: &HashMap<String, u16>) -> Result<Vec<DiffLine>> {
        let content = fs::read_to_string(&self.path)?;
        let mut diffs = Vec::new();

        for (i, line) in content.lines().enumerate() {
            let key = format!("line[{}]", i);
            if let Some(&new_port) = assignments.get(&key) {
                if Self::extract_port(line).is_some() {
                    let new_line = Self::replace_port(line, new_port);
                    diffs.push(DiffLine::Replaced {
                        old: line.to_string(),
                        new: new_line,
                    });
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
    fn test_extract_port_bare() {
        assert_eq!(NginxBackend::extract_port("listen 80;"), Some(80));
    }

    #[test]
    fn test_extract_port_bare_with_modifier() {
        assert_eq!(
            NginxBackend::extract_port("listen 80 default_server;"),
            Some(80)
        );
    }

    #[test]
    fn test_extract_port_ipv6() {
        assert_eq!(NginxBackend::extract_port("listen [::1]:80;"), Some(80));
    }

    #[test]
    fn test_extract_port_ipv6_with_modifier() {
        assert_eq!(
            NginxBackend::extract_port("listen [::1]:80 default_server;"),
            Some(80)
        );
    }

    #[test]
    fn test_extract_port_alt() {
        assert_eq!(NginxBackend::extract_port("listen 3000;"), Some(3000));
    }

    #[test]
    fn test_extract_port_indented() {
        assert_eq!(NginxBackend::extract_port("    listen 80;"), Some(80));
    }

    #[test]
    fn test_replace_port_bare() {
        assert_eq!(
            NginxBackend::replace_port("listen 80;", 3000),
            "listen 3000;"
        );
    }

    #[test]
    fn test_replace_port_bare_with_modifier() {
        assert_eq!(
            NginxBackend::replace_port("listen 80 default_server;", 3000),
            "listen 3000 default_server;"
        );
    }

    #[test]
    fn test_replace_port_ipv6() {
        assert_eq!(
            NginxBackend::replace_port("listen [::1]:80;", 3000),
            "listen [::1]:3000;"
        );
    }

    #[test]
    fn test_replace_port_ipv6_with_modifier() {
        assert_eq!(
            NginxBackend::replace_port("listen [::1]:80 default_server;", 3000),
            "listen [::1]:3000 default_server;"
        );
    }

    #[test]
    fn test_replace_port_roundtrip_bare() {
        let line = "listen 80;";
        let port = NginxBackend::extract_port(line).unwrap();
        let result = NginxBackend::replace_port(line, port);
        assert_eq!(result, line);
    }

    #[test]
    fn test_replace_port_roundtrip_ipv6() {
        let line = "listen [::1]:80;";
        let port = NginxBackend::extract_port(line).unwrap();
        let result = NginxBackend::replace_port(line, port);
        assert_eq!(result, line);
    }

    #[test]
    fn test_replace_port_roundtrip_ipv6_with_modifier() {
        let line = "listen [::1]:80 default_server;";
        let port = NginxBackend::extract_port(line).unwrap();
        let result = NginxBackend::replace_port(line, port);
        assert_eq!(result, line);
    }

    #[test]
    fn test_replace_port_indented() {
        assert_eq!(
            NginxBackend::replace_port("    listen [::1]:80;", 3000),
            "    listen [::1]:3000;"
        );
    }

    #[test]
    fn test_nginx_integration() {
        // ponytail: full read-write-read cycle with varied listen directives
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let content = "server {\n    listen 80;\n    listen [::1]:81;\n    listen 443 default_server;\n    listen [::1]:444 default_server;\n}\n";
        std::io::Write::write_all(&mut file, content.as_bytes()).unwrap();
        let path = file.path().to_path_buf();

        let backend = NginxBackend::new(&path);
        let ports = backend.read_ports().unwrap();
        assert_eq!(ports.len(), 4);

        // Verify each port and its metadata
        assert_eq!(ports[0].port, 80);
        assert_eq!(ports[1].port, 81);
        assert_eq!(ports[2].port, 443);
        assert_eq!(ports[3].port, 444);

        // Assign new ports
        let mut assignments = std::collections::HashMap::new();
        for p in &ports {
            assignments.insert(p.key.clone(), p.port + 1000);
        }

        // Write back
        let mut backend = NginxBackend::new(&path);
        backend.write_ports(&assignments).unwrap();

        // Read again and verify every port was bumped
        let backend = NginxBackend::new(&path);
        let ports2 = backend.read_ports().unwrap();
        assert_eq!(ports2.len(), 4);
        for p in &ports2 {
            let original = ports.iter().find(|op| op.key == p.key).unwrap();
            assert_eq!(p.port, original.port + 1000);
        }

        // Verify file content directly
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains("listen 1080;"));
        assert!(result.contains("listen [::1]:1081;"));
        assert!(result.contains("listen 1443 default_server;"));
        assert!(result.contains("listen [::1]:1444 default_server;"));
    }
}
