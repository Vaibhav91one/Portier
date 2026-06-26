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
    /// The entire detect → assign → persist cycle runs inside one
    /// [`Registry::update`] transaction, so concurrent CLI/MCP writes cannot
    /// interleave with the daemon's heal.
    pub fn step(&self) -> Result<usize> {
        let statuses = scanner::scan()?;
        let conflicts: Vec<_> = statuses.iter().filter(|s| s.is_conflict).collect();
        if conflicts.is_empty() {
            return Ok(0);
        }

        Registry::update(|reg| {
            let mut fixes = 0;

            for conflict in &conflicts {
                // Try to detect a registered project from each conflicting PID.
                for pid in &conflict.pids {
                    let Some((proj_name, _root)) = crate::detector::detect_project_from_pid(*pid)
                    else {
                        continue;
                    };
                    let Some((key, entry)) =
                        reg.find_by_name(&proj_name).map(|(k, e)| (k, e.clone()))
                    else {
                        continue;
                    };

                    let mut preferred_ports: HashMap<String, u16> = HashMap::new();
                    for (svc, svc_entry) in &entry.services {
                        preferred_ports.insert(svc.clone(), svc_entry.preferred);
                    }
                    // Track the conflicted port if no service claims it yet.
                    if !preferred_ports.contains_key("auto") {
                        preferred_ports.insert("auto".to_string(), conflict.port);
                    }

                    let assigner = Assigner::new(reg.clone());
                    if let Ok(assignments) = assigner.allocate(&preferred_ports, false, None) {
                        if let Some(project) = reg.get_project_mut(&key) {
                            for (svc, port) in &assignments {
                                project
                                    .services
                                    .entry(svc.clone())
                                    .or_insert(crate::registry::ServiceEntry {
                                        preferred: *port,
                                        assigned: *port,
                                        pid: Some(*pid),
                                    })
                                    .assigned = *port;
                            }
                            fixes += 1;
                        }
                    }
                    break;
                }
            }

            Ok(fixes)
        })
    }

    /// Run the daemon loop until stopped.
    pub fn run(&mut self) -> Result<()> {
        self.running = true;
        println!("Portier daemon started (interval: {}s)", self.interval_secs);

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
