# Portier — Contributor & Agent Guide

**Portier** is a cross-platform port-conflict manager. *Never fight a port conflict again.*

**The differentiator:** every other tool (portfinder, get-port, Portless) only *finds* a free port or *proxies* around the conflict. Portier **rewrites the actual config files** (docker-compose.yml, .env, nginx, package.json) so the project's real source of truth matches the assigned port — with snapshots for rollback. That is the moat. Do not trade it away for a proxy/`*.localhost` approach (that's Portless's game).

> This file documents Portier itself. `CLAUDE.md.agent` is a *different* artifact — a template Portier writes **into other people's repos** via `portier init-agent`. Don't confuse the two.

## Workspace

Rust workspace (`cargo`), four crates under `crates/`:

| Crate | Role |
|-------|------|
| `libportier` | Core library — all logic lives here, reused by the others |
| `portier-cli` | The `portier` binary (clap). Commands in `src/commands/` |
| `portier-mcp` | MCP server (`portier mcp`) — JSON-RPC over stdio for Claude Code / Cursor |
| `portier-tauri/src-tauri` | Tauri v2 desktop GUI (Rust backend + React/TS frontend in `../src`) |

`default-members` = `libportier` + `portier-cli`, so plain `cargo build`/`cargo test` skips the Tauri app.

## libportier modules

| Module | Responsibility |
|--------|----------------|
| `scanner` | Enumerate listening TCP ports via the **`listeners`** crate (native OS APIs — no shelling out). `scan() -> Vec<PortStatus>`. `build_statuses` is the pure, testable fold. |
| `registry` | Global state at `~/.config/portier/registry.json` (flock + atomic tmp+rename). **`Registry::update(\|r\| …)`** is the only safe read-modify-write. |
| `detector` | Single source of truth for "what/where is a project": `PROJECT_MARKERS` table, `detect_stack`, `resolve_project_root`, `detect_project_from_pid`, `find_common_project_root`. |
| `assigner` | Port allocation. `Assigner::allocate(...)` for multi-service; `pick_free_port(preferred, in_use, range)` for one (pure). |
| `config` | Per-project `portier.json` + `.env` PORT extraction. |
| `settings` | Global `~/.config/portier/config.toml` (`Settings::load()`): port range + `prefer_consecutive` + `avoid_well_known`. Optional; defaults to 3000–7000. Consumed by `assign`/`start`/`run`/`switch`. |
| `inject` | `framework_env_vars(command, port)` — env vars `portier run` injects into a child. |
| `rewriter/` | Config-file backends (docker, dotenv, nginx, generic) behind the `ConfigBackend` trait. Always snapshots before writing. |
| `snapshot` | Filesystem snapshots for `portier rollback`. |
| `daemon` | Optional polling auto-healer (`portier daemon`). |

## Invariants — do not break

1. **The registry is the single source of truth.** Every mutation goes through `Registry::update` (one lock across read-modify-write). Never `load()` → mutate → `save()` — that race loses concurrent writes. `load()`/`save()` are for read-only callers only.
2. **`scan()` is side-effect free** and must stay fast (~10ms, native APIs). The only intentional shell-out left is `detector::open_paths_for_pid` (macOS `lsof` for a process's *open files*, which `listeners` doesn't cover) — keep it isolated there.
3. **Config rewrites always snapshot first** (via the rewriter) so `portier rollback` can restore.
4. **Detection markers live in one place:** `detector::PROJECT_MARKERS`. Don't reintroduce a second marker list elsewhere.
5. **MCP tools return structured data.** Each tool emits `structuredContent` (typed via `schemars`) plus a short `text` summary. Agents read the structure — don't regress to prose-only.

## Commands

```
portier scan [--json]          # list listening ports + conflicts
portier link                   # register the cwd project
portier start [--yes]          # detect, allocate, rewrite configs, launch
portier run -- <cmd>           # allocate a free port + inject PORT, then run <cmd>
portier assign <name> [--dry-run]
portier settings [--init|--show]  # global ~/.config/portier/config.toml
portier status / stop / switch / rollback
portier daemon [--install]     # background auto-healer (launchd/systemd)
portier mcp                    # MCP server over stdio
portier init-agent             # write .mcp.json / .cursor rules / CLAUDE.md into a repo
```

## Build / test / run

```bash
cargo build                       # libportier + CLI
cargo test --workspace            # all crates
cargo build --release             # ~2 MB stripped binary (see [profile.release])
./target/debug/portier scan
cd crates/portier-tauri/src-tauri && cargo tauri dev   # GUI
```

MCP smoke test: pipe newline-delimited JSON-RPC (`initialize`, `tools/list`, `tools/call`) into `portier mcp`.

## Spec

Full product spec: `/Users/vaibhavtomar/Documents/app-specs/portier-port-manager.md`.
