# Portier 🔌

**Never fight a port conflict again.**

Portier is a cross-platform port-conflict manager. Other tools just *find* a free
port or *proxy* around the clash — Portier **rewrites the actual config files**
(`docker-compose.yml`, `.env`, `nginx`, `package.json`) so your project's real
source of truth matches the assigned port, with snapshots for instant rollback.

It ships as a CLI, an MCP server for AI agents (Claude Code / Cursor), and a
desktop GUI.

---

## Install

```bash
# From source
cargo install --path crates/portier-cli

# Homebrew (after a release is published)
brew install portier-cli/tap/portier
```

Pre-built binaries: [GitHub Releases](https://github.com/your-username/portier/releases).

## Quick start

```bash
portier scan                    # what's listening + any conflicts (no setup needed)
portier run -- npm run dev      # run a dev server on a guaranteed-free port
portier start                   # detect, assign free ports, rewrite configs, launch
```

`portier run` is the fastest path: it scans, picks a free port, injects `PORT`
(and rewrites `next -p` / `vite --port` / Django `runserver` flags), then runs
your command — so an agent-spawned dev server never hits `EADDRINUSE`.

## Commands

| Command | What it does |
|---|---|
| `portier scan [--json]` | List listening ports, highlight conflicts |
| `portier run -- <cmd>` | Allocate a free port + inject `PORT`, then run `<cmd>` |
| `portier start [--yes]` | Detect stack, assign ports, rewrite configs, launch |
| `portier link` | Register the current project |
| `portier status` | List registered projects and their ports |
| `portier assign <name> [--range A-B] [--dry-run]` | Assign ports to a project |
| `portier config init / show` | Per-project `portier.json` |
| `portier settings [--init / --show]` | Global `~/.config/portier/config.toml` |
| `portier stop / switch / rollback` | Lifecycle + restore from snapshot |
| `portier daemon [--install]` | Background auto-healer (launchd/systemd) |
| `portier init-agent` | Write `.mcp.json` / Cursor rules / `CLAUDE.md` into a repo |
| `portier mcp` | MCP server (JSON-RPC over stdio) |

## How it works

Portier keeps a registry at `~/.config/portier/registry.json`. On `start` it:

1. Detects your stack (Node, Python, Rust, Go, Ruby, Java, Docker).
2. Scans listening TCP ports via native OS APIs (the `listeners` crate — ~10 ms, no `lsof`).
3. Finds conflicts and allocates free ports.
4. **Rewrites the project's config files** to the new ports (snapshotted first).
5. Updates the registry.

Anything written can be undone with `portier rollback`.

## AI agent integration

Portier speaks MCP, so coding agents can resolve port conflicts on their own.
`portier init-agent` drops the right config into a repo; then agents can call:

- `scan_conflicts` — structured port + conflict data
- `allocate_port` — free port(s), no side effects
- `auto_heal` — assign + rewrite configs for a project
- `get_port_map` — current registry assignments

All tools return typed `structuredContent`, so agents read data instead of prose.

## Configuration

- **Per project**: `portier.json` (`portier config init`).
- **Global**: `~/.config/portier/config.toml` (`portier settings --init`):

```toml
[ranges]
start = 3000
end = 7000

[preferences]
prefer_consecutive = false
avoid_well_known = true
```

## Desktop GUI

A Tauri app (`crates/portier-tauri`) shows a live port map, project list,
conflict status, and lets you reassign ports — with light/dark themes.

```bash
cd crates/portier-tauri/src-tauri && cargo tauri dev
```

## Supported stacks

Node.js · Python · Ruby · Rust · Go · Java · Docker

## Development

```bash
cargo build              # libportier + CLI
cargo test --workspace   # all crates
cargo build --release    # ~2 MB stripped binary
```

See [`CLAUDE.md`](CLAUDE.md) for architecture and invariants.

## License

MIT
