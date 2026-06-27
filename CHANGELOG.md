# Changelog

## v0.1.0 (2026-06-27)

First release. Portier detects port conflicts and **rewrites your project's real
config files** so the assigned port becomes the source of truth — with snapshots
for instant rollback. Ships as a CLI, an MCP server for AI agents, and a desktop GUI.

### CLI

- `scan` — list listening ports and highlight conflicts (native OS APIs, ~10 ms)
- `run -- <cmd>` — allocate a free port, inject `PORT` (and rewrite `next -p` /
  `vite --port` / Django `runserver` flags), then run the command; auto-registers
  the project so it's tracked with a sticky port
- `start` — detect stack, assign free ports, rewrite configs, launch
- `link`, `status`, `assign`, `config`, `settings`, `stop`, `switch`, `rollback`
- `daemon` — background auto-healer (launchd/systemd)
- `init-agent` — write `.mcp.json` / Cursor rules / `CLAUDE.md` into a repo
- `mcp` — MCP server over stdio

### Core

- Config rewriting for docker-compose, `.env`, nginx, and generic JSON/YAML, with
  filesystem snapshots and `rollback`
- Native-API port scanning via the `listeners` crate (no shelling out to lsof/netstat)
- Atomic registry transactions (`Registry::update`) — no lost concurrent writes
- Global settings at `~/.config/portier/config.toml` (port range, prefer-consecutive,
  avoid-well-known)
- Projects auto-register when started via `portier run`/`start`

### AI agents

- MCP server built on the official `rmcp` SDK with typed, structured tool outputs:
  `scan_conflicts`, `allocate_port`, `auto_heal`, `get_port_map`

### Desktop GUI

- Tauri v2 + React app: tracked-project dashboard with live conflict status,
  per-service port editing, folder picker, dark mode

### Build

- ~1.7 MB stripped static binary, zero runtime dependencies
- CI builds + tests on macOS, Linux, and Windows; fmt + clippy gates
