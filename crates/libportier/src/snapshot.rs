use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub file_path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub project_root: String,
    pub files: Vec<SnapshotEntry>,
    pub timestamp: String,
}

pub fn take_snapshot(project_root: &Path, file_paths: &[String]) -> Result<Snapshot> {
    let id = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let mut files = Vec::new();
    for fp in file_paths {
        let full_path = project_root.join(fp);
        if full_path.exists() {
            let content = fs::read_to_string(&full_path)?;
            files.push(SnapshotEntry {
                file_path: fp.clone(),
                content,
            });
        }
    }
    Ok(Snapshot {
        id: id.clone(),
        project_root: project_root.to_string_lossy().to_string(),
        files,
        timestamp: id,
    })
}

pub fn save_snapshot(snapshot: &Snapshot, snapshots_dir: &Path) -> Result<()> {
    fs::create_dir_all(snapshots_dir)?;
    let json = serde_json::to_string_pretty(snapshot)?;
    let snapshot_file = snapshots_dir.join(format!("{}.json", snapshot.id));
    fs::write(&snapshot_file, &json)?;
    let content_dir = snapshots_dir.join(&snapshot.id);
    fs::create_dir_all(&content_dir)?;
    for entry in &snapshot.files {
        let fp = content_dir.join(sanitize_path(&entry.file_path));
        fs::write(&fp, &entry.content)?;
    }
    Ok(())
}

fn sanitize_path(path: &str) -> String {
    path.replace(['/', '\\'], "__")
}

pub fn get_snapshots(snapshots_dir: &Path) -> Result<Vec<String>> {
    if !snapshots_dir.exists() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in fs::read_dir(snapshots_dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|e| e == "json") {
            if let Some(stem) = entry.path().file_stem() {
                ids.push(stem.to_string_lossy().to_string());
            }
        }
    }
    ids.sort();
    ids.reverse();
    Ok(ids)
}

pub fn load_snapshot(snapshot_id: &str, snapshots_dir: &Path) -> Result<Snapshot> {
    let snapshot_file = snapshots_dir.join(format!("{}.json", snapshot_id));
    let json = fs::read_to_string(&snapshot_file)?;
    let snapshot: Snapshot = serde_json::from_str(&json)?;
    Ok(snapshot)
}

pub fn delete_snapshot(snapshot_id: &str, snapshots_dir: &Path) -> Result<()> {
    let snapshot_file = snapshots_dir.join(format!("{}.json", snapshot_id));
    let _ = fs::remove_file(&snapshot_file);
    let content_dir = snapshots_dir.join(snapshot_id);
    let _ = fs::remove_dir_all(&content_dir);
    Ok(())
}

pub fn restore_snapshot(snapshot: &Snapshot) -> Result<()> {
    let root = Path::new(&snapshot.project_root);
    for entry in &snapshot.files {
        let full_path = root.join(&entry.file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, &entry.content)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_restore_snapshot() {
        let tmp = TempDir::new().unwrap();
        let tmp_path = tmp.path().to_path_buf();

        // Create a file with original content
        let original_content = "hello world";
        let file_path = "test.txt";
        std::fs::write(tmp_path.join(file_path), original_content).unwrap();

        // Take a snapshot
        let snapshot = take_snapshot(&tmp_path, &[file_path.to_string()]).unwrap();

        // Modify the file
        std::fs::write(tmp_path.join(file_path), "modified content").unwrap();

        // Restore the snapshot
        restore_snapshot(&snapshot).unwrap();

        // Verify the file content matches the original
        let restored = std::fs::read_to_string(tmp_path.join(file_path)).unwrap();
        assert_eq!(restored, original_content);
    }

    #[test]
    fn test_snapshot_persistence_round_trip() {
        let proj = TempDir::new().unwrap();
        let store = TempDir::new().unwrap();
        std::fs::write(proj.path().join(".env"), "PORT=3000\n").unwrap();

        // take -> save
        let snap = take_snapshot(proj.path(), &[".env".to_string()]).unwrap();
        save_snapshot(&snap, store.path()).unwrap();

        // get lists the id
        let ids = get_snapshots(store.path()).unwrap();
        assert!(ids.contains(&snap.id));

        // load reads it back intact
        let loaded = load_snapshot(&snap.id, store.path()).unwrap();
        assert_eq!(loaded.files.len(), 1);
        assert_eq!(loaded.files[0].file_path, ".env");
        assert_eq!(loaded.files[0].content, "PORT=3000\n");

        // delete removes it
        delete_snapshot(&snap.id, store.path()).unwrap();
        assert!(!get_snapshots(store.path()).unwrap().contains(&snap.id));
    }

    #[test]
    fn test_take_snapshot_skips_missing_files() {
        let proj = TempDir::new().unwrap();
        let snap = take_snapshot(proj.path(), &["does-not-exist.env".to_string()]).unwrap();
        assert!(snap.files.is_empty());
    }

    #[test]
    fn test_restore_snapshot_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let tmp_path = tmp.path().to_path_buf();

        // Take a snapshot with a file in a subdirectory that doesn't exist yet
        let snapshot = Snapshot {
            id: "test".to_string(),
            project_root: tmp_path.to_string_lossy().to_string(),
            files: vec![SnapshotEntry {
                file_path: "sub/dir/test.txt".to_string(),
                content: "nested content".to_string(),
            }],
            timestamp: "test".to_string(),
        };

        restore_snapshot(&snapshot).unwrap();

        let restored = std::fs::read_to_string(tmp_path.join("sub/dir/test.txt")).unwrap();
        assert_eq!(restored, "nested content");
    }
}
