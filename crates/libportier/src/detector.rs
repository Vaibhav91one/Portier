use crate::error::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ProjectStack {
    Node,
    Python,
    Ruby,
    Rust,
    Go,
    Java,
    Docker,
    Unknown,
}

impl std::fmt::Display for ProjectStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectStack::Node => write!(f, "Node"),
            ProjectStack::Python => write!(f, "Python"),
            ProjectStack::Ruby => write!(f, "Ruby"),
            ProjectStack::Rust => write!(f, "Rust"),
            ProjectStack::Go => write!(f, "Go"),
            ProjectStack::Java => write!(f, "Java"),
            ProjectStack::Docker => write!(f, "Docker"),
            ProjectStack::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectionResult {
    pub stack: ProjectStack,
    pub config_files: Vec<String>,
    pub project_root: String,
}

/// The single source of truth for "what marks a project root", paired with the
/// stack each marker implies. Order matters: `detect_stack` picks the first
/// match, so language-specific markers precede generic ones (`Makefile`).
///
/// Used by both stack classification ([`detect_stack`]) and root discovery
/// ([`resolve_project_root`]) so the two can never diverge. `.git` is handled
/// separately in `resolve_project_root` (it's a directory, not a stack marker).
pub const PROJECT_MARKERS: &[(&str, ProjectStack)] = &[
    ("package.json", ProjectStack::Node),
    ("requirements.txt", ProjectStack::Python),
    ("pyproject.toml", ProjectStack::Python),
    ("Pipfile", ProjectStack::Python),
    ("Gemfile", ProjectStack::Ruby),
    ("Cargo.toml", ProjectStack::Rust),
    ("go.mod", ProjectStack::Go),
    ("pom.xml", ProjectStack::Java),
    ("build.gradle", ProjectStack::Java),
    ("docker-compose.yml", ProjectStack::Docker),
    ("Makefile", ProjectStack::Unknown),
];

pub fn detect_stack(root: &Path) -> Result<DetectionResult> {
    let mut found_files = Vec::new();
    let mut stack = ProjectStack::Unknown;

    for (marker, detected_stack) in PROJECT_MARKERS {
        if root.join(marker).exists() {
            found_files.push(marker.to_string());
            if matches!(stack, ProjectStack::Unknown) {
                stack = detected_stack.clone();
            }
        }
    }

    Ok(DetectionResult {
        stack,
        config_files: found_files,
        project_root: root.to_string_lossy().to_string(),
    })
}

pub fn find_config_files(root: &Path) -> Vec<String> {
    let patterns = [
        ".env",
        "docker-compose.yml",
        "package.json",
        "nginx.conf",
        "application.yml",
    ];
    let mut files: Vec<String> = patterns
        .iter()
        .filter(|p| root.join(p).exists())
        .map(|p| p.to_string())
        .collect();

    // ponytail: read_dir scan for *.nginx.conf instead of pulling in glob crate
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".nginx.conf") {
                    files.push(name.to_string());
                }
            }
        }
    }

    files
}

// ---------------------------------------------------------------------------
// Project root + PID resolution
//
// (Moved here from scanner.rs so all "what/where is the project" logic shares
// one marker table. The macOS open-FD listing is the only remaining shell-out
// in this module — isolated in `open_paths_for_pid` so it can later be swapped
// for libproc without touching the resolution logic.)
// ---------------------------------------------------------------------------

/// Walk up from `start` to find the topmost project root.
///
/// A directory qualifies if it contains any [`PROJECT_MARKERS`] file or a
/// `.git` directory. Returns the highest qualifying ancestor — except a `.git`
/// directory is treated as the definitive root and stops the walk.
pub fn resolve_project_root(start: &Path) -> Option<(String, PathBuf)> {
    let mut best: Option<(String, PathBuf)> = None;
    let mut dir = Some(start.to_path_buf());

    while let Some(ref mut d) = dir {
        let has_marker = PROJECT_MARKERS.iter().any(|(m, _)| d.join(m).exists());
        let has_git = d.join(".git").exists();

        if has_marker || has_git {
            let name = d.file_name()?.to_string_lossy().to_string();
            best = Some((name, d.clone()));
            // .git marks the definitive root — don't climb past it.
            if has_git {
                break;
            }
        }

        if !d.pop() {
            break;
        }
        dir = Some(d.clone());
    }

    best
}

/// Return the filesystem paths a process currently has open.
///
/// This is the module's lone intentional shell-out (`lsof -p <PID> -Fn` on
/// macOS). The `listeners` crate covers listening *sockets* but not a process's
/// open *files*, which is what we need to locate its project directory. On
/// Linux/Windows we derive paths from procfs / the executable path instead, so
/// this returns empty there and the caller uses those sources directly.
#[cfg(target_os = "macos")]
fn open_paths_for_pid(pid: u32) -> Vec<String> {
    let output = match std::process::Command::new("lsof")
        .args(["-p", &pid.to_string(), "-Fn"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let stdout = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix('n'))
        .filter(|p| p.starts_with('/') && *p != "/")
        .map(|p| p.to_string())
        .collect()
}

/// Detect project identity from a running PID.
///
/// Returns `Some((project_name, project_root_path))` or `None`. Works WITHOUT
/// the project being registered in Portier's registry.
///
/// - **macOS**: scans the process's open files. A `/node_modules/` path points
///   at its parent project; otherwise filters out system paths and walks for
///   markers. In both cases climbs to the topmost project root.
/// - **Linux**: `/proc/<pid>/cwd`, falling back to the executable's directory.
/// - **Windows**: the executable's directory via `wmic`.
pub fn detect_project_from_pid(pid: u32) -> Option<(String, PathBuf)> {
    #[cfg(target_os = "macos")]
    {
        let paths = open_paths_for_pid(pid);
        if paths.is_empty() {
            return None;
        }

        // Strategy 1: a node_modules path -> its parent is the project.
        const NEEDLE: &str = "/node_modules/";
        for p in &paths {
            if let Some(pos) = p.find(NEEDLE) {
                let dir: PathBuf = p[..pos].into();
                if let Some(result) = resolve_project_root(&dir) {
                    return Some(result);
                }
                let name = dir.file_name()?.to_string_lossy().to_string();
                return Some((name, dir));
            }
        }

        // Strategy 2: filter user paths, find the deepest marker dir, climb up.
        fn is_user_path(p: &str) -> bool {
            let sys_prefixes = [
                "/usr",
                "/System",
                "/Library",
                "/private/var",
                "/dev",
                "/etc",
                "/opt",
            ];
            !sys_prefixes.iter().any(|pfx| p.starts_with(pfx))
        }

        let user_paths: Vec<&String> = paths.iter().filter(|p| is_user_path(p)).collect();
        let candidates: Vec<&String> = if user_paths.is_empty() {
            paths.iter().collect()
        } else {
            user_paths
        };

        for p in candidates {
            let mut dir: PathBuf = p.as_str().into();
            if dir
                .file_name()
                .and_then(|n| Path::new(n).extension())
                .is_some()
            {
                dir.pop();
            }
            let mut deepest: Option<PathBuf> = None;
            loop {
                if PROJECT_MARKERS.iter().any(|(m, _)| dir.join(m).exists()) {
                    deepest = Some(dir.clone());
                }
                if !dir.pop() {
                    break;
                }
            }
            if let Some(d) = deepest {
                if let Some(result) = resolve_project_root(&d) {
                    return Some(result);
                }
                let name = d.file_name()?.to_string_lossy().to_string();
                return Some((name, d));
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    {
        let cwd = std::fs::read_link(format!("/proc/{pid}/cwd")).ok()?;
        if let Some(result) = resolve_project_root(&cwd) {
            return Some(result);
        }
        if let Ok(exe) = std::fs::read_link(format!("/proc/{pid}/exe")) {
            if let Some(parent) = exe.parent() {
                if let Some(result) = resolve_project_root(parent) {
                    return Some(result);
                }
            }
        }
        None
    }

    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("wmic")
            .args([
                "process",
                "where",
                &format!("processid={pid}"),
                "get",
                "ExecutablePath",
            ])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout);
        let path = s.lines().nth(1).map(|l| PathBuf::from(l.trim()))?;
        if let Some(parent) = path.parent() {
            if let Some(result) = resolve_project_root(parent) {
                return Some(result);
            }
        }
        None
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = pid;
        None
    }
}

/// Resolve each path to its project root and return the most common one.
pub fn find_common_project_root(paths: &[PathBuf]) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }

    let roots: Vec<PathBuf> = paths
        .iter()
        .filter_map(|p| resolve_project_root(p).map(|(_, r)| r))
        .collect();
    if roots.is_empty() {
        return None;
    }

    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for r in &roots {
        *counts.entry(r.to_string_lossy().to_string()).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(key, _)| PathBuf::from(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_detect_node() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        let result = detect_stack(dir.path()).unwrap();
        assert_eq!(result.stack, ProjectStack::Node);
        assert!(result.config_files.contains(&"package.json".to_string()));
    }

    #[test]
    fn test_detect_rust() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let result = detect_stack(dir.path()).unwrap();
        assert_eq!(result.stack, ProjectStack::Rust);
    }

    #[test]
    fn test_detect_python() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("requirements.txt"), "").unwrap();
        let result = detect_stack(dir.path()).unwrap();
        assert_eq!(result.stack, ProjectStack::Python);
    }

    #[test]
    fn test_detect_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let result = detect_stack(dir.path()).unwrap();
        assert_eq!(result.stack, ProjectStack::Unknown);
        assert!(result.config_files.is_empty());
    }

    #[test]
    fn test_detect_first_match_wins() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let result = detect_stack(dir.path()).unwrap();
        // package.json checked first -> Node
        assert_eq!(result.stack, ProjectStack::Node);
        assert!(result.config_files.contains(&"Cargo.toml".to_string()));
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", ProjectStack::Node), "Node");
        assert_eq!(format!("{}", ProjectStack::Rust), "Rust");
        assert_eq!(format!("{}", ProjectStack::Unknown), "Unknown");
    }

    #[test]
    fn test_find_config_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".env"), "").unwrap();
        let files = find_config_files(dir.path());
        assert!(files.contains(&".env".to_string()));
    }

    #[test]
    fn test_find_nginx_conf_glob() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("myapp.nginx.conf"), "").unwrap();
        let files = find_config_files(dir.path());
        assert!(files.contains(&"myapp.nginx.conf".to_string()));
    }

    #[test]
    fn test_resolve_project_root_marker() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let (name, root) = resolve_project_root(dir.path()).unwrap();
        assert_eq!(root, dir.path());
        assert_eq!(name, dir.path().file_name().unwrap().to_string_lossy());
    }

    #[test]
    fn test_resolve_project_root_none_for_bare_dir() {
        let dir = tempfile::tempdir().unwrap();
        // No markers, no .git anywhere up to the temp root.
        assert!(resolve_project_root(dir.path()).is_none());
    }

    #[test]
    fn test_resolve_project_root_git_stops_climb() {
        // outer/.git and outer/inner/Cargo.toml — .git is the definitive root,
        // so resolving from inner climbs up to outer and stops there.
        let outer = tempfile::tempdir().unwrap();
        fs::create_dir(outer.path().join(".git")).unwrap();
        let inner = outer.path().join("inner");
        fs::create_dir(&inner).unwrap();
        fs::write(inner.join("Cargo.toml"), "").unwrap();

        let (_, root) = resolve_project_root(&inner).unwrap();
        assert_eq!(root, outer.path());
    }

    #[test]
    fn test_markers_table_drives_detect_stack() {
        // A Makefile-only dir is a root but classifies as Unknown stack.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Makefile"), "").unwrap();
        assert!(resolve_project_root(dir.path()).is_some());
        let result = detect_stack(dir.path()).unwrap();
        assert_eq!(result.stack, ProjectStack::Unknown);
        assert!(result.config_files.contains(&"Makefile".to_string()));
    }
}
