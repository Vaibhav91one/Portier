// Port scanning via the `listeners` crate, which uses native OS APIs
// (libproc on macOS, /proc/net/tcp on Linux, IP Helper API on Windows) to
// enumerate sockets with their owning PID and process name in one call — no
// subprocess, no procfs parsing. This replaces the prior lsof/netstat/procfs
// shell-outs and matches the original spec's mandate.

use std::collections::HashMap;

use serde::Serialize;

use crate::error::{PortierError, Result};

#[derive(Debug, Clone, Serialize)]
pub struct PortStatus {
    pub port: u16,
    pub pids: Vec<u32>,
    pub process_names: Vec<String>,
    pub is_conflict: bool,
}

/// Scan the local system for all TCP ports in LISTEN state.
///
/// Returns a deduplicated snapshot: each entry maps a port to the set of PIDs
/// (and their process names) found listening on it. When more than one
/// unique PID is present, `is_conflict` is `true`.
pub fn scan() -> Result<Vec<PortStatus>> {
    let sockets =
        listeners::get_all().map_err(|e| PortierError::Other(format!("port scan failed: {e}")))?;

    let raw: Vec<(u16, u32, String)> = sockets
        .into_iter()
        .filter(|l| {
            l.protocol == listeners::Protocol::TCP && l.state == listeners::SocketState::Listen
        })
        .map(|l| {
            let name = if l.process.name.is_empty() {
                format!("unknown-{}", l.process.pid)
            } else {
                l.process.name
            };
            (l.socket.port(), l.process.pid, name)
        })
        .collect();

    Ok(build_statuses(raw))
}

/// Fold raw `(port, pid, name)` tuples into deduplicated `PortStatus` entries.
///
/// Pure (no syscalls) so it is unit-testable directly. A port is a conflict
/// only when more than one *unique* PID listens on it — the IPv4 and IPv6
/// entries a single process opens collapse to one claim.
fn build_statuses(raw: Vec<(u16, u32, String)>) -> Vec<PortStatus> {
    let mut ports: HashMap<u16, (Vec<u32>, Vec<String>)> = HashMap::new();

    for (port, pid, name) in raw {
        let (pids, names) = ports.entry(port).or_insert_with(|| (Vec::new(), Vec::new()));
        if !pids.contains(&pid) {
            pids.push(pid);
            names.push(name);
        }
    }

    let mut result: Vec<PortStatus> = ports
        .into_iter()
        .map(|(port, (pids, process_names))| {
            let is_conflict = pids.len() > 1;
            PortStatus {
                port,
                pids,
                process_names,
                is_conflict,
            }
        })
        .collect();

    result.sort_by_key(|p| p.port);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_returns_vec() {
        let result = scan().unwrap();
        assert!(result.iter().all(|s| s.port > 0));
    }

    #[test]
    fn test_scan_sorted() {
        let result = scan().unwrap();
        for w in result.windows(2) {
            assert!(w[0].port <= w[1].port);
        }
    }

    #[test]
    fn test_port_status_fields() {
        let ps = PortStatus {
            port: 8080,
            pids: vec![100, 200],
            process_names: vec!["nginx".into(), "apache".into()],
            is_conflict: true,
        };
        assert_eq!(ps.port, 8080);
        assert_eq!(ps.pids.len(), 2);
        assert!(ps.is_conflict);
    }

    #[test]
    fn test_port_status_no_conflict() {
        let ps = PortStatus {
            port: 3000,
            pids: vec![42],
            process_names: vec!["node".into()],
            is_conflict: false,
        };
        assert!(!ps.is_conflict);
    }

    #[test]
    fn test_build_statuses_dedups_same_pid() {
        // Same PID bound on IPv4 and IPv6 for one port = a single claim,
        // not a conflict.
        let raw = vec![
            (8080, 100, "nginx".to_string()),
            (8080, 100, "nginx".to_string()),
        ];
        let out = build_statuses(raw);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pids, vec![100]);
        assert!(!out[0].is_conflict);
    }

    #[test]
    fn test_build_statuses_detects_conflict() {
        // Two distinct PIDs on the same port = conflict.
        let raw = vec![
            (3000, 100, "node".to_string()),
            (3000, 200, "next".to_string()),
        ];
        let out = build_statuses(raw);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pids.len(), 2);
        assert!(out[0].is_conflict);
    }

    #[test]
    fn test_build_statuses_sorted_by_port() {
        let raw = vec![
            (9000, 1, "a".to_string()),
            (3000, 2, "b".to_string()),
            (5000, 3, "c".to_string()),
        ];
        let out = build_statuses(raw);
        let ports: Vec<u16> = out.iter().map(|s| s.port).collect();
        assert_eq!(ports, vec![3000, 5000, 9000]);
    }
}
