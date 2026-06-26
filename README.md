# Portier 🔌

**Never fight a port conflict again.**

Portier detects port conflicts, assigns free ports, and manages your project's port configurations automatically.

## Features

- **Scan** — List all listening ports and detect conflicts
- **Link** — Register projects for global port management
- **Start** — Scan, assign free ports, update registry, all in one command
- **Assign** — Allocate free ports from configurable ranges
- **Status** — View all registered projects and their port assignments
- **Config** — Initialize and manage per-project config (`portier.json`)
- **Switch** — Switch between projects, freeing and assigning ports
- **Stop** — Stop project services and free ports

## Installation

### From source

```bash
cargo install --path .
```

### Pre-built binaries

Download from [GitHub Releases](https://github.com/your-username/portier/releases).

## Usage

```bash
# Scan for port conflicts in the current directory
portier scan

# Scan system-wide with JSON output
portier scan --all --json

# Link the current project in the global registry
portier link

# Initialize project config
portier config init --name my-project

# Show registered projects
portier status

# Start project with auto-assignment
portier start

# Assign specific ports
portier assign my-project --range 3000-4000

# Switch to another project
portier switch other-project

# Stop project services
portier stop
```

## How it works

Portier maintains a registry at `~/.config/portier/registry.json` that tracks all registered projects and their port assignments. When you run `portier start`, it:

1. Detects your project stack (Node, Python, Rust, etc.)
2. Scans all listening TCP ports on your machine
3. Detects conflicts (multiple processes on the same port)
4. Assigns free ports from the preferred range
5. Updates the registry with new assignments

## Configuration

Portier uses a `~/.config/portier/registry.json` file for global project tracking and optional `portier.json` per project for detailed port configuration.

## Supported Stacks

- Node.js (package.json)
- Python (requirements.txt, pyproject.toml, Pipfile)
- Ruby (Gemfile)
- Rust (Cargo.toml)
- Go (go.mod)
- Java (pom.xml, build.gradle)
- Docker (docker-compose.yml)

## License

MIT
