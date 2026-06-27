# Portier — Launch Kit

Internal launch notes. Not shipped to users.

## The one-line wedge

Every other port tool either **finds** a free port (get-port, portfinder) or
**proxies** around the clash with stable URLs (Portless). Portier **rewrites the
real config files** — docker-compose, .env, nginx, package.json — so the assigned
port becomes the project's source of truth, with snapshots for rollback.

A proxy hides the conflict. Portier fixes the files.

## Why now (validated 2026-06-27)

- get-port (~19M/wk) + portfinder (~12M/wk) = tens of millions of weekly installs
  for the bare "find a free port" primitive. The demand is enormous and proven.
- Vercel Labs' `portless` is at ~10K stars — "for humans and agents." The category
  is hot and the agent framing is mainstream.
- A crowd of 2026 entrants (portless-rs, dockportless, a Go portfinder, Port-Kill)
  means builders smell users.
- Nobody auto-rewrites the source config on conflict — Docker's own answer is
  manual override files. **That lane is open.**

## Show HN

**Title options (pick one):**
1. `Show HN: Portier – a port-conflict manager that rewrites your config, not a proxy`
2. `Show HN: Portier – end localhost port conflicts by editing the real config files`
3. `Show HN: Portier – give every dev server a free port, and update docker-compose/.env to match`

**Body:**

> Portier is a small Rust CLI that fixes localhost port conflicts by rewriting the
> files that actually declare the port — docker-compose.yml, .env, nginx, package.json —
> instead of finding a random free port at runtime or proxying behind a *.localhost URL.
>
> The flow that matters: `portier run -- npm run dev`. It scans listening ports
> (native OS APIs, ~10ms, no lsof), picks a free one, injects PORT (and rewrites
> `next -p` / `vite --port` / Django runserver args), runs your command, and
> registers the project so the port is sticky next time. `portier start` goes
> further and rewrites your config files to the assigned ports, snapshotting first
> so `portier rollback` can undo everything.
>
> Why rewrite the config at all? Because the conflict keeps coming back otherwise.
> A proxy or a runtime free-port works until you commit, share the repo, or hand it
> to a teammate whose machine has a different set of ports taken. Portier makes the
> repo's declared port correct, then lets you roll it back.
>
> The reason I built it now: AI agents. When Claude Code / Cursor spin up a dev
> server, they hit EADDRINUSE and flail. Portier ships an MCP server (official rmcp
> SDK, typed structured outputs) so an agent can call `auto_heal` / `allocate_port`
> directly, plus `portier init-agent` to wire it into a repo.
>
> Rust, ~1.7MB static binary, zero runtime deps. macOS/Linux/Windows. MIT.
>
> Honest about what it is NOT: it edits config files, so it's opinionated about
> your repo. There's a daemon for auto-healing but on-demand is the default. Config
> rewriting covers the common formats; exotic ones fall back to a generic JSON/YAML
> pass. Feedback very welcome, especially on the rewrite-vs-proxy tradeoff.
>
> Repo: https://github.com/Vaibhav91one/Portier

## Portier vs the field

| | get-port / portfinder | Portless (+ clones) | **Portier** |
|---|---|---|---|
| Approach | Find a free port at runtime | Reverse-proxy to stable `*.localhost` URLs | **Rewrite the real config files** |
| Source of truth | unchanged (still says 3000) | unchanged (hidden behind URL) | **updated to the assigned port** |
| Survives commit / teammate | no | no | **yes (config is correct in the repo)** |
| Rollback | n/a | n/a | **snapshots + `portier rollback`** |
| Agent integration | library call | proxy + agent flag | **MCP server (typed tools) + `init-agent`** |
| Footprint | npm lib | proxy daemon | **~1.7MB static binary, 0 deps** |

## Talking points / FAQ to pre-empt

- **"Why not just use a proxy like Portless?"** Proxies are great for stable URLs;
  they don't fix the conflict in the repo. Portier is for when you want the declared
  port to actually be free-and-correct, and to undo it cleanly. They compose fine.
- **"Editing my config files sounds scary."** Every write is snapshotted; `portier
  rollback` restores. Dry-run shows the diff first.
- **"Does it work without setup?"** `portier scan` and `portier run` need zero config.
- **Two-line install:** Homebrew tap / `npm i -g portier-cli` / cargo install.

## Channels (in priority order)

1. Show HN (devtools land here — the whole 30-day conversation was Show-HNs + agent threads)
2. r/webdev, r/node, r/devops, r/commandline
3. The AI-agent angle: a short clip of an agent self-healing a port conflict via MCP
