use crate::error::Result;
use crate::registry::Registry;
use crate::scanner::scan;
use std::collections::{HashMap, HashSet};

/// Pick a single free port for one service.
///
/// Returns `preferred` when it is free, otherwise the lowest free port in
/// `range` (default `3000..7000`). Falls back to `preferred` if the whole
/// range is exhausted. Pure given `in_use`, so it is directly testable and is
/// shared by `portier run` and the MCP `allocate_port` tool.
pub fn pick_free_port(
    preferred: u16,
    in_use: &HashSet<u16>,
    range: Option<std::ops::Range<u16>>,
) -> u16 {
    if !in_use.contains(&preferred) {
        return preferred;
    }
    for port in range.unwrap_or(3000u16..7000u16) {
        if !in_use.contains(&port) {
            return port;
        }
    }
    preferred
}

pub struct Assigner {
    pub registry: Registry,
}

impl Assigner {
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }

    /// Allocate ports for services. Returns map of service_name -> assigned_port.
    /// `range` controls the free-port search range; defaults to 3000..7000.
    pub fn allocate(
        &self,
        preferred_ports: &HashMap<String, u16>,
        prefer_consecutive: bool,
        range: Option<std::ops::Range<u16>>,
    ) -> Result<HashMap<String, u16>> {
        let current_claims = scan()?;
        let in_use: HashSet<u16> = current_claims.iter().map(|p| p.port).collect();
        Ok(self.allocate_inner(preferred_ports, prefer_consecutive, &in_use, range))
    }

    /// Internal allocation that accepts a pre-built in_use set (testable).
    fn allocate_inner(
        &self,
        preferred_ports: &HashMap<String, u16>,
        prefer_consecutive: bool,
        in_use: &HashSet<u16>,
        range: Option<std::ops::Range<u16>>,
    ) -> HashMap<String, u16> {
        let search_range = range.unwrap_or(3000u16..7000u16);
        let mut assigned = HashMap::new();
        let mut used_in_session = HashSet::new();

        if prefer_consecutive {
            // Collect services sorted by preferred port (ascending)
            let mut services: Vec<(&String, &u16)> = preferred_ports.iter().collect();
            services.sort_by(|a, b| a.1.cmp(b.1));

            let mut prev_port: Option<u16> = None;

            for (service, &preferred) in &services {
                let port = if let Some(prev) = prev_port {
                    let candidate = prev + 1;
                    if !in_use.contains(&candidate) && !used_in_session.contains(&candidate) {
                        candidate
                    } else {
                        self.find_free_port(search_range.clone(), in_use, &used_in_session)
                            .unwrap_or(preferred)
                    }
                } else {
                    // First service: try preferred, else find_free_port
                    if !in_use.contains(&preferred) && !used_in_session.contains(&preferred) {
                        preferred
                    } else {
                        self.find_free_port(search_range.clone(), in_use, &used_in_session)
                            .unwrap_or(preferred)
                    }
                };

                assigned.insert((*service).clone(), port);
                used_in_session.insert(port);
                prev_port = Some(port);
            }
        } else {
            for (service, &preferred) in preferred_ports {
                if !in_use.contains(&preferred) && !used_in_session.contains(&preferred) {
                    assigned.insert(service.clone(), preferred);
                    used_in_session.insert(preferred);
                    continue;
                }

                // Find free port in range
                let port = self.find_free_port(
                    search_range.clone(),
                    in_use,
                    &used_in_session,
                );
                if let Some(p) = port {
                    assigned.insert(service.clone(), p);
                    used_in_session.insert(p);
                } else {
                    // Fallback: just assign preferred even if conflicted
                    assigned.insert(service.clone(), preferred);
                }
            }
        }

        assigned
    }

    fn find_free_port(
        &self,
        range: std::ops::Range<u16>,
        in_use: &HashSet<u16>,
        used_in_session: &HashSet<u16>,
    ) -> Option<u16> {
        for port in range {
            if !in_use.contains(&port) && !used_in_session.contains(&port) {
                return Some(port);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_find_free_port_basic() {
        let registry = Registry::new();
        let assigner = Assigner::new(registry);
        let in_use: HashSet<u16> = [3000, 3001, 3002].iter().cloned().collect();
        let used: HashSet<u16> = HashSet::new();

        let port = assigner.find_free_port(3000..4000, &in_use, &used);
        assert_eq!(port, Some(3003));
    }

    #[test]
    fn test_find_free_port_all_used() {
        let registry = Registry::new();
        let assigner = Assigner::new(registry);
        let in_use: HashSet<u16> = (1000..2000).collect();
        let used: HashSet<u16> = HashSet::new();

        let port = assigner.find_free_port(1000..2000, &in_use, &used);
        assert_eq!(port, None);
    }

    #[test]
    fn test_find_free_port_respects_session_usage() {
        let registry = Registry::new();
        let assigner = Assigner::new(registry);
        let in_use: HashSet<u16> = HashSet::new();
        let used: HashSet<u16> = [4000].iter().cloned().collect();

        let port = assigner.find_free_port(4000..4010, &in_use, &used);
        assert_eq!(port, Some(4001));
    }

    #[test]
    fn test_allocate_simple() {
        // This test won't call scan() since allocate() calls scan() internally
        // and scan() uses real ports. This tests find_free_port directly instead.
        let registry = Registry::new();
        let assigner = Assigner::new(registry);

        let mut preferred = HashMap::new();
        preferred.insert("web".to_string(), 3000);

        // allocate() calls scan() which is a real syscall — smoke test only
        let result = assigner.allocate(&preferred, false, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_allocate_consecutive_assigns_consecutive_ports() {
        let registry = Registry::new();
        let assigner = Assigner::new(registry);

        let mut preferred = HashMap::new();
        preferred.insert("web".to_string(), 3000);
        preferred.insert("api".to_string(), 4000);
        preferred.insert("db".to_string(), 5000);

        // No system ports in use — all preferred and consecutive are available
        let in_use: HashSet<u16> = HashSet::new();
        let result = assigner.allocate_inner(&preferred, true, &in_use, None);

        assert_eq!(result.len(), 3);
        assert_eq!(*result.get("web").unwrap(), 3000);
        assert_eq!(*result.get("api").unwrap(), 3001);
        assert_eq!(*result.get("db").unwrap(), 3002);
    }

    #[test]
    fn test_allocate_consecutive_skips_in_use_ports() {
        let registry = Registry::new();
        let assigner = Assigner::new(registry);

        let mut preferred = HashMap::new();
        preferred.insert("web".to_string(), 3000);
        preferred.insert("api".to_string(), 4000);
        preferred.insert("db".to_string(), 5000);

        // 3001 is taken on the system
        let in_use: HashSet<u16> = [3001].iter().cloned().collect();
        let result = assigner.allocate_inner(&preferred, true, &in_use, None);

        assert_eq!(result.len(), 3);
        assert_eq!(*result.get("web").unwrap(), 3000);
        // 3001 is in use, so api falls back to find_free_port (first free >= 3000)
        assert_eq!(*result.get("api").unwrap(), 3002);
        // db should try 3003 (api + 1) since 3002 is used
        assert_eq!(*result.get("db").unwrap(), 3003);
    }

    #[test]
    fn test_allocate_non_consecutive_keeps_preferred() {
        let registry = Registry::new();
        let assigner = Assigner::new(registry);

        let mut preferred = HashMap::new();
        preferred.insert("web".to_string(), 3000);
        preferred.insert("api".to_string(), 4000);
        preferred.insert("db".to_string(), 5000);

        let in_use: HashSet<u16> = HashSet::new();
        let result = assigner.allocate_inner(&preferred, false, &in_use, None);

        assert_eq!(result.len(), 3);
        assert_eq!(*result.get("web").unwrap(), 3000);
        assert_eq!(*result.get("api").unwrap(), 4000);
        assert_eq!(*result.get("db").unwrap(), 5000);
    }

    #[test]
    fn test_pick_free_port_preferred_free() {
        let in_use: HashSet<u16> = [3001, 3002].iter().cloned().collect();
        assert_eq!(pick_free_port(3000, &in_use, None), 3000);
    }

    #[test]
    fn test_pick_free_port_preferred_taken() {
        let in_use: HashSet<u16> = [3000, 3001].iter().cloned().collect();
        assert_eq!(pick_free_port(3000, &in_use, Some(3000..3010)), 3002);
    }

    #[test]
    fn test_pick_free_port_range_exhausted_falls_back() {
        let in_use: HashSet<u16> = (3000..3010).collect();
        assert_eq!(pick_free_port(3000, &in_use, Some(3000..3010)), 3000);
    }

    #[test]
    fn test_allocate_consecutive_first_preferred_taken() {
        let registry = Registry::new();
        let assigner = Assigner::new(registry);

        let mut preferred = HashMap::new();
        preferred.insert("web".to_string(), 3000);
        preferred.insert("api".to_string(), 4000);

        // 3000 is taken on the system
        let in_use: HashSet<u16> = [3000].iter().cloned().collect();
        let result = assigner.allocate_inner(&preferred, true, &in_use, None);

        assert_eq!(result.len(), 2);
        // web gets 3001 (first free after 3000)
        assert_eq!(*result.get("web").unwrap(), 3001);
        // api tries 3002, not 4000
        assert_eq!(*result.get("api").unwrap(), 3002);
    }
}
