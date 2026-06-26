pub mod error;
pub mod scanner;
pub mod registry;
pub mod detector;
pub mod config;
pub mod assigner;
pub mod inject;
pub mod rewriter;
pub mod snapshot;
pub mod daemon;

pub use error::{PortierError, Result};
pub use scanner::{PortStatus, scan};
pub use registry::{Registry, ProjectEntry, ServiceEntry};
pub use detector::{
    detect_project_from_pid, find_common_project_root, resolve_project_root, DetectionResult,
    ProjectStack,
};
pub use config::ProjectConfig;
pub use assigner::{pick_free_port, Assigner};
pub use inject::framework_env_vars;
pub use rewriter::{ConfigBackend, DiffLine, PortDeclaration};
pub use snapshot::{Snapshot, take_snapshot, save_snapshot, load_snapshot, delete_snapshot, get_snapshots, restore_snapshot};
