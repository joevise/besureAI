# 🐉 Besure AI Context

### Local-first context memory for AI agents and humans.

[English](README.md) | [中文](README_CN.md)

> 貔貅 (Píxiū), a mythical beast that only takes in and never loses — symbolizing memory that, once stored, is never forgotten.

**Rust-powered · Local-first · End-to-end encrypted · CLI-native · Single binary**

**Current version: 0.62.0** — Real semantic search: local fastembed (bge-small-zh-v1.5, 512-dim) runs fully offline — zero API cost, zero key, privacy-safe. `besure index` builds the vector index; `besure add` auto-indexes incrementally.

---

## Why Besure AI Context?

You work on multiple projects. You switch between tasks. Every time you switch, you lose context — what you were doing, what you decided, what you learned. AI agents have the same problem.

**Besure AI Context fixes this:**

| Problem | Solution |
|---------|----------|
| 🔀 **Context loss** when switching projects | Git-branch-like context isolation — work on one, switch instantly |
| 🤖 **AI agents can't remember** across sessions | CLI-native integration — agents exec `besure` commands to store & retrieve context |
| 🔐 **Multiple agents, no isolation** | Multi-vault architecture — each agent gets its own physically isolated vault |
| ☁️ **Cloud dependency & privacy concerns** | 100% local — SQLite + Markdown, zero cloud required |
| 🔓 **Data security** | AES-256-GCM + Argon2id encryption — keys never touch disk |
| 📦 **Setup complexity** | Single binary, zero runtime dependencies — `curl | bash` and you're done |

---

## Core Concepts (V0.58)

Besure AI Context has exactly **three** core concepts — nothing else:

| Concept | What it is |
|---------|-----------|
| **Context** | An isolated memory space (like a git branch). One per project/task. |
| **Entry** | A single memory record inside a context. Everything is an entry: progress, decisions, milestones, blockers, notes, lessons, questions. |
| **Auto Tags** | Every entry is automatically tagged with 1–3 broad flat tags by an LLM at `besure add` time (synchronous). Tags are emergent: a shared `tag_vocab` vocabulary reuses semantically identical tags to prevent synonym explosion. |

> **No Config concept.** As of V0.58, there is no separate "config" feature — everything you used to store as config is just a regular entry, organized and found via auto tags. (App-level LLM/embedding provider settings live in `~/.besure/appconfig.json`, managed via `besure appconfig`.)

### Semantic Search (V0.61)

Semantic search runs **100% locally** via the embedded [`fastembed`](https://crates.io/crates/fastembed) engine with the **bge-small-zh-v1.5** model (512-dim, Chinese-friendly). No API, no key, no cost — works fully offline, and your data never leaves the machine.

- **Auto incremental indexing**: every `besure add` embeds the new entry into `vectors.db` (synchronous, degrades gracefully if the model is unavailable — add is never blocked).
- **Backfill existing data**: `besure index --all` embeds all existing entries (skips already-indexed ones; use `--rebuild` to redo).
- **Search**: `besure search "semantic description" --semantic` finds memories by meaning, not keywords. Also available via REST (`GET /api/search?q=...&semantic=true`, vault-scoped `GET /api/vaults/:id/search?...&semantic=true`) and the Dashboard (语义搜索 toggle).
- First run downloads the model (~100MB) to the HuggingFace cache (`~/.cache/huggingface`); afterwards it loads from disk in ~1-2s.

---

## Quick Start

### Install

```bash
# One-line install (macOS / Linux)
curl -fsSL https://raw.githubusercontent.com/joevise/besureAI/main/install.sh | bash

# Or via cargo
cargo install besure

# Or download pre-built binary
# → https://github.com/joevise/besureAI/releases
```

After installation, the Dashboard auto-starts as a background service with crash recovery and boot persistence:

| Platform | Mechanism | Auto-managed via |
|----------|-----------|------------------|
| **Linux** | systemd user service | `systemctl --user {start|stop|status} besure-dashboard` |
| **macOS** | launchd agent | `launchctl {load|unload} ~/Library/LaunchAgents/com.besure.context.plist` |
| **Windows** | Startup folder + VBS | Auto-starts on login |

```bash
besure service install     # Install background service
besure service status      # Check if Dashboard is running
besure service uninstall   # Remove background service
```

### Use

```bash
# Initialize with encryption
besure init --encrypt

# Create your first context
besure create "My Project" --tag rust --summary "Building something cool"

# Record progress
besure add "Implemented auth module" --type milestone
besure add "Decided to use Axum" --type decision
besure add "Hit a compile error, fixed" --type blocker

# List all contexts
besure list

# Search across everything
besure search "auth"

# Switch between contexts (fuzzy match)
besure switch "project"

# Unified query with filters (V0.4)
besure query                              # Latest 20 entries
besure query --last 7d                     # Last 7 days
besure query --type decision               # Only decisions
besure query --all --keyword "auth"        # All contexts, keyword filter

# Mark entry as resolved
besure resolve <entry_id>

# Recycle Bin (V0.60): soft delete → trash → restore / purge
besure delete context <id>           # Move context (+ its entries) to trash
besure delete entry <id>             # Move entry to trash
besure trash                         # View trash contents
besure restore <id>                  # Restore context or entry from trash
besure purge <id>                    # Permanently delete (irreversible)

# Export a context (encrypted .besure, default)
besure export "My Project" --password *** -o backup.besure
besure export "My Project" -o backup.besure        # interactive password prompt

# Legacy Markdown export
besure export "My Project" --format md -o project-context.md

# Import an encrypted .besure file (entries deduped by id)
besure import backup.besure --password ***
```

### Multi-Vault: One Vault Per Agent (V0.5)

Each AI agent gets its own physically isolated vault. Agents cannot see each other's data unless explicitly shared.

```bash
# Set up vault per agent via environment variable
export BESURE_VAULT=~/.besure/joey          # Joey's vault
export BESURE_VAULTS_ALL=true               # Grant global view (master agent only)

# Other agents get their own vault, no global view
export BESURE_VAULT=~/.besure/coding-agent  # Coding agent's vault

# List all vaults (requires BESURE_VAULTS_ALL=true)
besure vaults

# Query across all vaults
besure query --all-vaults

# Share an entry to the shared vault
besure share <entry_id>

# View shared content
besure shared
```

---

## Connect to AI Agents (CLI)

Besure AI Context is CLI-native. Any AI agent that can execute shell commands (Claude Code, OpenClaw, Cursor, Codex, etc.) can store and retrieve context by running `besure` commands directly.

### How it works

1. **Inject the mandatory recording rules** — add the `skill/SKILL.md` iron rules to your agent's config (AGENTS.md / CLAUDE.md / .cursorrules). This tells the agent WHEN to record (after every task, decision, commit, deploy).
2. **The agent execs `besure` commands** — no protocol, no server, no extra process. Each call is an independent `besure` invocation.

Or run one command to do both (init + inject rules into detected agent config files):

```bash
besure setup --agent-name "Joey" --agent-type openclaw
```

Now your AI agent can:
- **List contexts** → `besure list` — see all your projects
- **Add entries** → `besure add "..." --type decision --context <id>` (auto-tagged by LLM)
- **Search memory** → `besure search "..."` / `besure search "..." --semantic`
- **Query with filters** → `besure query --last 7d --type decision` (V0.4)
- **List tags** → `besure tags` (V0.58)
- **Resolve entries** → `besure resolve <entry_id>`
- **Append to entries** → `besure append <entry_id> "..."`
- **View stats** → `besure stats` (V0.58: By Tag is primary)
- **Multi-vault** → isolated vaults per agent via `BESURE_VAULT`, shared vault for collaboration (V0.5)
- **Create contexts** → `besure create "..."`
- **Export & share** → `besure export` / `besure import` / `besure share`

See the [CLI Reference](#cli-reference) below for the full command list, and `skill/SKILL.md` for the agent-facing skill document.

---

## Web Dashboard

```bash
besure serve --port 7788
# → Open http://localhost:7788
```

A built-in web UI for browsing contexts, viewing timelines, filtering by tag, and managing entries. The Stats page is organized **By Tag** (V0.58).

**Dashboard password security:**
- If your vault is encrypted, the Dashboard requires your master password.
- For unencrypted vaults (or to use a separate Dashboard-only password), set the environment variable `BESURE_DASHBOARD_PASSWORD` before starting the server — it takes priority over vault auth.
- ⚠️ If the vault is unencrypted **and** `BESURE_DASHBOARD_PASSWORD` is not set, the Dashboard accepts any password (insecure — a warning is printed on startup). Always set one of the two when exposing the Dashboard beyond localhost.

---

## Security

| Feature | Detail |
|---------|--------|
| **Encryption** | AES-256-GCM (military-grade, authenticated) |
| **Key derivation** | Argon2id (64MB memory / 3 iterations / 4 threads — GPU-cracking resistant) |
| **Key storage** | Keys exist only in memory, zeroized on lock — never written to disk |
| **File-level encryption** | Each file encrypted independently — single file leak doesn't compromise others |
| **Auth** | Dashboard requires master password — same key encrypts data and authorizes access |
| **No cloud** | Zero network calls to external services. Your data never leaves your machine. |

---

## CLI Reference

```
# === Vault ===
besure init --encrypt             Initialize vault with encryption
besure unlock                     Unlock vault
besure lock                       Lock vault
besure status                     Show vault status

# === Context ===
besure create <title>             Create a new context
besure switch <query>             Switch context (fuzzy match)
besure list                       List all contexts

# === Entries ===
besure add <content>              Add entry (--type, --from-file)
besure log [context]              View timeline
besure search <query>             Full-text search (--semantic for local vector search)
besure index [--all]              Build semantic vector index (local fastembed, offline)
besure absorb [--auto]            Extract entries from conversation text

# === Query & Manage (V0.4) ===
besure query                      Unified query (time/type/keyword/resolved filters)
  --last 7d                       Last N days
  --from / --to                   Date range
  --type <t>                      Filter by type (repeatable)
  --all                           Search all contexts
  --keyword <kw>                  Keyword filter
  --unresolved / --resolved       Resolved filter
  --limit <n>                     Max results (default 20)
besure resolve <entry_id>         Mark entry as resolved
besure append <entry_id> <text>  Append content to existing entry
besure stats                      Statistics overview

# === Multi-Vault (V0.5) ===
besure vaults                     List all vaults (requires BESURE_VAULTS_ALL=true)
besure query --all-vaults         Query across all vaults
besure share <entry_id>           Share entry to shared vault
besure share-context <ctx_id>     Share entire context
besure shared [--keyword <kw>]    View shared vault contents

# === Auto-Tagging (V0.58) ===
besure add <content>              Auto-tags entry with 1-3 broad tags (sync, LLM)
besure tags                       Show tag vocabulary (tag + usage count)
besure retag [--all] [--context <id>]  Re-tag existing entries

# === App Config (LLM / embedding providers) ===
besure appconfig <key> <value>    Set app-level config, e.g.:
                                  llm.provider / llm.api_url / llm.api_key / llm.model
                                  embedding.provider / embedding.api_url / embedding.api_key / embedding.model

# --- Recommended LLM for auto-tagging: OpenRouter + DeepSeek V4 Flash (cheap & fast) ---
# Get your own key at https://openrouter.ai/keys, then:
besure appconfig llm.provider openrouter
besure appconfig llm.api_url https://openrouter.ai/api/v1/chat/completions
besure appconfig llm.api_key sk-or-v1-YOUR_OWN_KEY
besure appconfig llm.model deepseek/deepseek-v4-flash

# === Closure (V3) ===
besure link <id> --to <id>        Link entries (caused_by/supersedes/related_to)
besure expire <id>                Mark entry as expired
besure supersede <old> <new>      Mark old entry superseded by new
besure recall                     Recall entries needing attention

# === Recycle Bin (V0.60) ===
besure delete context <id>        Move context (+ entries) to trash
besure delete entry <id>          Move entry to trash
besure trash                      View trash contents
besure restore <id>               Restore context or entry from trash
besure purge <id>                 Permanently delete (irreversible)

# === Server ===
besure serve [--port 7788]        Start web dashboard + REST API
besure export <context>           Export to encrypted .besure (default)
besure export <context> --format md   Export to Markdown (legacy)
besure import <file.besure>       Import encrypted .besure (dedupes by id)
```

---

## Architecture

```
┌──────────────────────────────────────────┐
│          Besure AI Context                │
│                                          │
│  ┌────────────────────────────────────┐  │
│  │         Interface Layer            │  │
│  │  CLI · REST API · Web Dashboard     │  │
│  └──────────────┬─────────────────────┘  │
│                 │                        │
│  ┌──────────────▼─────────────────────┐  │
│  │          Engine Layer              │  │
│  │  Context Mgmt · Search · Absorb    │  │
│  └──────────────┬─────────────────────┘  │
│                 │                        │
│  ┌──────────────▼─────────────────────┐  │
│  │          Storage Layer             │  │
│  │  SQLite · Markdown · Vector Store  │  │
│  └────────────────────────────────────┘  │
│                                          │
│  ┌────────────────────────────────────┐  │
│  │          Crypto Layer              │  │
│  │  AES-256-GCM · Argon2id · Zeroize  │  │
│  └────────────────────────────────────┘  │
└──────────────────────────────────────────┘

Single binary. Zero external dependencies. Pure Rust.
```

---

## How It Compares

| | Besure AI Context | Obsidian | Notion | Mem.ai |
|---|---|---|---|---|
| **AI Agent integration** | CLI (exec) | None | Limited | API only |
| **Context isolation** | Built-in (git-branch model) | Manual | Workspaces | Tags |
| **Deployment** | Local-first, single binary | Local app | Cloud-only | Cloud-only |
| **Encryption** | E2E, AES-256-GCM | No | No | At-rest only |
| **Language** | Rust | JS/TS | JS/TS | JS/TS |
| **Open source** | ✅ MIT | ❌ | ❌ | ❌ |

---

## Roadmap

| Phase | Status | Features |
|-------|--------|----------|
| **MVP** | ✅ Done | Crypto engine, SQLite, CLI, Markdown files |
| **V1-V2** | ✅ Done | Vector search, MCP server (8 tools), Absorb, REST API, Web Dashboard with auth |
| **V3** | ✅ Done | Closure engine: entry links, expiry, supersede, recall (16 MCP tools) |
| **V0.4** | ✅ Done | Unified query (time/type/keyword/resolved filters), resolve, append, stats (20 MCP tools) |
| **V0.5** | ✅ Done | Multi-vault architecture: physical isolation per agent, global view, shared vault (23 MCP tools) |
| **V0.5.5** | ✅ Done | Dashboard multi-Agent view: sidebar Agent list, data source switching |
| **V0.56** | ✅ Done | `besure setup` + mandatory recording rules: multi-platform detection, idempotent injection |
| **V0.58** | ✅ Done | Emergent auto-tagging: removed Config concept — everything is now entries + auto flat broad tags. LLM tags every entry on add (1-3 tags, sync), `tag_vocab` table with synonym reuse, `besure tags` / `besure retag`, Dashboard Stats now By Tag, Dashboard auth fix (BESURE_DASHBOARD_PASSWORD) (20 MCP tools) |
| **V0.59** | ✅ Done | Encrypted export/import: `.besure` format (AES-256-GCM + Argon2id, not a zip — cannot be opened by any tool without password). `besure export --password` / `besure import --password`, vault-scoped REST endpoints, Dashboard Export/Import UI (21 MCP tools) |
| **V0.60** | ✅ Done | Recycle Bin: soft delete contexts/entries to trash, restore or permanently purge. `besure delete/restore/trash/purge`, Dashboard Trash view, all list/stats/query exclude deleted items (23 MCP tools) |
| **V0.61** | ✅ Done | Real semantic search: local fastembed + bge-small-zh-v1.5 (512-dim), fully offline/zero-cost/zero-key. `besure index`, auto-index on add, `search --semantic`, MCP `semantic` param, REST `?semantic=true`, Dashboard semantic toggle |
| **V0.62** | ✅ Done | Removed MCP layer: agents integrate via CLI (exec `besure` commands) + SKILL.md iron rules. `besure mcp` command removed |
| **Next** | 📋 Planned | Tauri desktop app, crates.io publish, GitHub Actions CI, Product Hunt launch |
| **Future** | 📋 Planned | VS Code extension, browser extension, team collaboration |

---

## Tech Stack

| Component | Crate |
|-----------|-------|
| Encryption | `aes-gcm` + `argon2` + `zeroize` |
| Database | `rusqlite` (SQLite bundled into binary) |
| CLI | `clap` |
| HTTP Client | `reqwest` (rustls-tls, no OpenSSL dependency) |
| REST API | `axum` + `tokio` |
| Serialization | `serde` + `serde_json` |

**100% Rust. No Python runtime. No Node.js. No system libraries. One binary.**

---

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT — see [LICENSE](LICENSE).

## Links

- **GitHub**: [github.com/joevise/besureAI](https://github.com/joevise/besureAI)
- **Releases**: [Download pre-built binaries](https://github.com/joevise/besureAI/releases)
- **Design Doc**: [DESIGN.md](DESIGN.md) (Chinese, English translation coming soon)

---

*Besure AI Context — Once stored, never lost. 🐉*
