# Changelog

## v0.1.0 (2026-06-26)

Initial release with the following commands:

- **scan** -- Scan local ports and detect active processes
- **assign** -- Assign a port to a process or service
- **link** -- Link a host:port to a service
- **start** -- Start a portier-managed service
- **stop** -- Stop a portier-managed service
- **switch** -- Switch traffic between port assignments
- **status** -- Show current port assignments and service state
- **config** -- Manage portier configuration
- **rollback** -- Roll back to a previous snapshot

Key features:

- Config rewriting for docker-compose, .env, nginx, and generic JSON/YAML files
- Snapshot-based rollback
- File locking for safe concurrent access
