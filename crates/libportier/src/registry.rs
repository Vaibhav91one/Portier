// ponytail: flock guards against concurrent portier processes.
//           Atomic write via tmp+rename prevents partial reads.

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::path::PathBuf;

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::error::{PortierError, Result};

/// A guard that holds an exclusive file lock. The lock is released on drop.
pub struct FileLock {
    file: File,
}

impl FileLock {
    fn new(file: File) -> std::io::Result<Self> {
        file.lock_exclusive()?;
        Ok(Self { file })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // ponytail: unlock errors are benign — file closes shortly after.
        let _ = self.file.unlock();
    }
}

/// A service within a project, with its port preferences and runtime PID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    pub preferred: u16,
    pub assigned: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

/// A registered project with its stack, services, and optional metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub stack: String,
    pub services: HashMap<String, ServiceEntry>,
    #[serde(default)]
    pub linked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
}

/// The global portier registry, stored at ~/.config/portier/registry.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub version: u32,
    pub projects: HashMap<String, ProjectEntry>,
}

impl Registry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            version: 1,
            projects: HashMap::new(),
        }
    }

    /// Return the on-disk path for the registry file.
    pub fn registry_path() -> Result<PathBuf> {
        let home = dirs_next().ok_or_else(|| PortierError::NotFound("HOME not set".into()))?;
        Ok(home.join(".config").join("portier").join("registry.json"))
    }

    /// Acquire an exclusive advisory lock on the registry, blocking until acquired.
    fn lock_file() -> Result<FileLock> {
        let path = Self::registry_path()?;
        let lock_path = path.with_extension("lock");
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&lock_path)?;
        FileLock::new(file)
            .map_err(|e| PortierError::Registry(format!("failed to acquire registry lock: {e}")))
    }

    /// Load the registry from disk, or return a fresh empty one if missing.
    ///
    /// Acquires and releases the lock for the read. For read-modify-write
    /// cycles use [`Registry::update`] instead — `load` then `save` leaves an
    /// unlocked window in which another process can interleave a write.
    pub fn load() -> Result<Self> {
        let _lock = Self::lock_file()?;
        Self::load_locked()
    }

    /// Save the registry to disk atomically (write tmp, rename).
    ///
    /// Acquires and releases the lock for the write. See [`Registry::update`]
    /// for atomic read-modify-write.
    pub fn save(&self) -> Result<()> {
        let _lock = Self::lock_file()?;
        self.save_locked()
    }

    /// Atomic read-modify-write under a single exclusive lock.
    ///
    /// The closure mutates the registry in place; the result is persisted
    /// (tmp + rename) before the lock releases. If the closure returns `Err`,
    /// nothing is written and the on-disk file is left untouched. Returns the
    /// closure's value.
    ///
    /// This is the only safe way to update the registry — `load()` followed by
    /// `save()` releases the lock in between and can lose concurrent writes.
    pub fn update<T>(f: impl FnOnce(&mut Registry) -> Result<T>) -> Result<T> {
        let _lock = Self::lock_file()?;
        let mut reg = Self::load_locked()?;
        let out = f(&mut reg)?;
        reg.save_locked()?;
        Ok(out)
    }

    /// Read the registry assuming the lock is already held.
    fn load_locked() -> Result<Self> {
        let path = Self::registry_path()?;
        if !path.exists() {
            return Ok(Self::new());
        }
        let data = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&data)?)
    }

    /// Write the registry assuming the lock is already held.
    fn save_locked(&self) -> Result<()> {
        let path = Self::registry_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, &data)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    // -- Project CRUD -------------------------------------------------------

    pub fn get_project(&self, path: &str) -> Option<&ProjectEntry> {
        self.projects.get(path)
    }

    pub fn get_project_mut(&mut self, path: &str) -> Option<&mut ProjectEntry> {
        self.projects.get_mut(path)
    }

    pub fn add_project(&mut self, path: String, entry: ProjectEntry) {
        self.projects.insert(path, entry);
    }

    pub fn remove_project(&mut self, path: &str) {
        self.projects.remove(path);
    }

    /// Find a project by its `name` field (not path key).
    pub fn find_by_name(&self, name: &str) -> Option<(String, &ProjectEntry)> {
        self.projects
            .iter()
            .find(|(_, e)| e.name == name)
            .map(|(k, v)| (k.clone(), v))
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    /// Serializes tests that modify HOME to prevent env-var races.
    /// ponytail: global lock, thread-local HOME if env var races recur.
    static HOME_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_registry_new_empty() {
        let reg = Registry::new();
        assert_eq!(reg.version, 1);
        assert!(reg.projects.is_empty());
    }

    #[test]
    fn test_registry_crud() {
        let mut reg = Registry::new();
        let entry = ProjectEntry {
            name: "test-project".into(),
            stack: "Node".into(),
            services: HashMap::new(),
            linked: true,
            worktree: Some("/tmp/test".into()),
        };

        reg.add_project("/tmp/test".into(), entry);
        assert_eq!(reg.projects.len(), 1);

        let found = reg.get_project("/tmp/test");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test-project");

        let by_name = reg.find_by_name("test-project");
        assert!(by_name.is_some());

        reg.remove_project("/tmp/test");
        assert!(reg.projects.is_empty());
    }

    #[test]
    fn test_registry_roundtrip() {
        let _home_lock = HOME_LOCK.lock().unwrap();

        // Override HOME to a temp dir
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().to_string_lossy().to_string();

        // Temporarily swap HOME
        let original_home = env::var("HOME").ok();
        env::set_var("HOME", &home);

        let mut reg = Registry::new();
        reg.add_project(
            "/tmp/test".into(),
            ProjectEntry {
                name: "roundtrip".into(),
                stack: "Rust".into(),
                services: {
                    let mut m = HashMap::new();
                    m.insert(
                        "web".into(),
                        ServiceEntry {
                            preferred: 3000,
                            assigned: 3001,
                            pid: None,
                        },
                    );
                    m
                },
                linked: true,
                worktree: None,
            },
        );

        // Save and reload
        reg.save().unwrap();
        let loaded = Registry::load().unwrap();
        assert_eq!(loaded.projects.len(), 1);
        let entry = loaded.get_project("/tmp/test").unwrap();
        assert_eq!(entry.name, "roundtrip");
        assert_eq!(entry.services["web"].preferred, 3000);
        assert_eq!(entry.services["web"].assigned, 3001);

        // Cleanup
        if let Some(h) = original_home {
            env::set_var("HOME", h);
        }
    }

    #[test]
    fn test_service_entry_defaults() {
        let entry = ServiceEntry {
            preferred: 8080,
            assigned: 8080,
            pid: Some(12345),
        };
        assert_eq!(entry.preferred, 8080);
        assert_eq!(entry.pid, Some(12345));
    }

    #[test]
    fn test_concurrent_access() {
        let _home_lock = HOME_LOCK.lock().unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().to_string_lossy().to_string();
        let original_home = env::var("HOME").ok();
        env::set_var("HOME", &home);

        // Seed the registry with an initial project
        let mut reg = Registry::new();
        reg.add_project(
            "/initial".into(),
            ProjectEntry {
                name: "initial".into(),
                stack: "Rust".into(),
                services: HashMap::new(),
                linked: false,
                worktree: None,
            },
        );
        reg.save().unwrap();

        // Two threads each add a distinct project via the atomic transaction.
        let h1 = std::thread::spawn(|| {
            Registry::update(|r| {
                r.add_project(
                    "/thread1".into(),
                    ProjectEntry {
                        name: "thread1".into(),
                        stack: "Go".into(),
                        services: HashMap::new(),
                        linked: false,
                        worktree: None,
                    },
                );
                Ok(())
            })
            .unwrap();
        });

        let h2 = std::thread::spawn(|| {
            Registry::update(|r| {
                r.add_project(
                    "/thread2".into(),
                    ProjectEntry {
                        name: "thread2".into(),
                        stack: "Py".into(),
                        services: HashMap::new(),
                        linked: false,
                        worktree: None,
                    },
                );
                Ok(())
            })
            .unwrap();
        });

        h1.join().unwrap();
        h2.join().unwrap();

        // With Registry::update holding one lock across read-modify-write,
        // neither thread can clobber the other: ALL three projects survive.
        let loaded = Registry::load().unwrap();
        assert!(loaded.get_project("/initial").is_some(), "initial entry must survive");
        assert!(loaded.get_project("/thread1").is_some(), "thread1 must not be lost");
        assert!(loaded.get_project("/thread2").is_some(), "thread2 must not be lost");

        // The file is valid JSON.
        let path = Registry::registry_path().unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        serde_json::from_str::<serde_json::Value>(&raw)
            .expect("registry file must be valid JSON");

        if let Some(h) = original_home {
            env::set_var("HOME", h);
        }
    }

    #[test]
    fn test_update_aborts_on_err() {
        let _home_lock = HOME_LOCK.lock().unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().to_string_lossy().to_string();
        let original_home = env::var("HOME").ok();
        env::set_var("HOME", &home);

        // Seed one project.
        Registry::update(|r| {
            r.add_project(
                "/keep".into(),
                ProjectEntry {
                    name: "keep".into(),
                    stack: "Rust".into(),
                    services: HashMap::new(),
                    linked: false,
                    worktree: None,
                },
            );
            Ok(())
        })
        .unwrap();

        // A transaction that mutates then returns Err must NOT persist.
        let result: Result<()> = Registry::update(|r| {
            r.add_project(
                "/discard".into(),
                ProjectEntry {
                    name: "discard".into(),
                    stack: "Go".into(),
                    services: HashMap::new(),
                    linked: false,
                    worktree: None,
                },
            );
            Err(PortierError::Registry("boom".into()))
        });
        assert!(result.is_err());

        let loaded = Registry::load().unwrap();
        assert!(loaded.get_project("/keep").is_some(), "committed entry survives");
        assert!(
            loaded.get_project("/discard").is_none(),
            "aborted transaction must not persist"
        );

        if let Some(h) = original_home {
            env::set_var("HOME", h);
        }
    }
}

/// Resolve the user's home directory.
fn dirs_next() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home));
    }
    #[cfg(target_os = "windows")]
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return Some(PathBuf::from(profile));
    }
    None
}
