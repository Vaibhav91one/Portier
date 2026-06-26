// ponytail: simple polling daemon — no fancy process monitoring, no system extension.
// Upgrade to notify-based file watching if config-change latency matters.

use crate::assigner::Assigner;
use crate::error::Result;
use crate::registry::Registry;
use crate::scanner;
use std::collections::HashMap;
use std::time::Duration;

/// Simple background daemon that detects and auto-heals port conflicts.
pub struct Daemon {
    pub interval_secs: u64,
    pub running: bool,
}

impl Daemon {
    pub fn new(interval_secs: u64) -> Self {
        Self {
            interval_secs,
            running: false,
        }
    }

    /// Single scan + heal cycle. Returns count of conflicts fixed.
    ///
    /// Scanning and PID→project resolution are read-only and happen first; the
    /// allocate → persist step then runs inside one [`Registry::update`]
    /// transaction, so concurrent CLI/MCP writes can't interleave.
    pub fn step(&self) -> Result<usize> {
        let statuses = scanner::scan()?;
        let conflicts: Vec<_> = statuses.iter().filter(|s| s.is_conflict).collect();
        if conflicts.is_empty() {
            return Ok(0);
        }

        // Resolve each conflicting port to a registered project (read-only).
        let mut targets: Vec<(String, u16)> = Vec::new();
        for conflict in &conflicts {
            for pid in &conflict.pids {
                if let Some((proj_name, _root)) = crate::detector::detect_project_from_pid(*pid) {
                    targets.push((proj_name, conflict.port));
                    break;
                }
            }
        }
        if targets.is_empty() {
            return Ok(0);
        }

        Registry::update(|reg| Ok(Self::heal(reg, &targets)))
    }

    /// Reassign ports for each `(project_name, conflicted_port)` target against
    /// the given registry, mutating it in place. Returns the number of projects
    /// healed. Pure with respect to the OS except for the free-port scan inside
    /// the allocator, so it is unit-testable with a synthetic registry.
    pub fn heal(reg: &mut Registry, targets: &[(String, u16)]) -> usize {
        let mut fixes = 0;
        for (proj_name, port) in targets {
            let Some((key, entry)) = reg.find_by_name(proj_name).map(|(k, e)| (k, e.clone()))
            else {
                continue;
            };

            let mut preferred_ports: HashMap<String, u16> = HashMap::new();
            for (svc, svc_entry) in &entry.services {
                preferred_ports.insert(svc.clone(), svc_entry.preferred);
            }
            // Track the conflicted port if no service claims it yet.
            preferred_ports.entry("auto".to_string()).or_insert(*port);

            let assigner = Assigner::new(reg.clone());
            if let Ok(assignments) = assigner.allocate(&preferred_ports, false, None) {
                if let Some(project) = reg.get_project_mut(&key) {
                    for (svc, p) in &assignments {
                        project
                            .services
                            .entry(svc.clone())
                            .or_insert(crate::registry::ServiceEntry {
                                preferred: *p,
                                assigned: *p,
                                pid: None,
                            })
                            .assigned = *p;
                    }
                    fixes += 1;
                }
            }
        }
        fixes
    }

    /// Run the daemon loop until stopped.
    pub fn run(&mut self) -> Result<()> {
        self.running = true;
        println!("Portier daemon started (interval: {}s)", self.interval_secs);

        // `running` is toggled by `stop()` from another thread / signal handler,
        // so the condition is not mutated within this loop body by design.
        #[allow(clippy::while_immutable_condition)]
        while self.running {
            match self.step() {
                Ok(fixes) => {
                    if fixes > 0 {
                        println!("  Fixed {} port conflict(s)", fixes);
                    }
                }
                Err(e) => {
                    eprintln!("Daemon step error: {}", e);
                }
            }
            std::thread::sleep(Duration::from_secs(self.interval_secs));
        }

        println!("Portier daemon stopped.");
        Ok(())
    }

    pub fn stop(&mut self) {
        self.running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{ProjectEntry, ServiceEntry};

    fn registry_with_project(name: &str, service: &str, preferred: u16) -> Registry {
        let mut reg = Registry::new();
        let mut services = HashMap::new();
        services.insert(
            service.to_string(),
            ServiceEntry {
                preferred,
                assigned: preferred,
                pid: None,
            },
        );
        reg.add_project(
            format!("/tmp/{name}"),
            ProjectEntry {
                name: name.to_string(),
                stack: "Node".to_string(),
                services,
                linked: true,
                worktree: Some(format!("/tmp/{name}")),
            },
        );
        reg
    }

    #[test]
    fn test_new_sets_fields() {
        let d = Daemon::new(5);
        assert_eq!(d.interval_secs, 5);
        assert!(!d.running);
    }

    #[test]
    fn test_heal_no_targets_is_noop() {
        let mut reg = registry_with_project("demo", "web", 3000);
        let fixes = Daemon::heal(&mut reg, &[]);
        assert_eq!(fixes, 0);
        assert_eq!(
            reg.get_project("/tmp/demo").unwrap().services["web"].assigned,
            3000
        );
    }

    #[test]
    fn test_heal_unknown_project_is_noop() {
        let mut reg = registry_with_project("demo", "web", 3000);
        let fixes = Daemon::heal(&mut reg, &[("ghost".to_string(), 3000)]);
        assert_eq!(fixes, 0);
    }

    #[test]
    fn test_heal_reassigns_registered_project() {
        // Reserve the project's preferred port so the allocator must move it.
        let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let busy_port = busy.local_addr().unwrap().port();

        let mut reg = registry_with_project("demo", "web", busy_port);
        let fixes = Daemon::heal(&mut reg, &[("demo".to_string(), busy_port)]);

        assert_eq!(fixes, 1);
        let assigned = reg.get_project("/tmp/demo").unwrap().services["web"].assigned;
        assert_ne!(assigned, busy_port, "should move web off the busy port");
        assert!((3000..7000).contains(&assigned));
    }
}
