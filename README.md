# 🐉 Besure AI Context

### Local-first context memory for AI agents and humans.

[English](README.md) | [中文](README_CN.md)

> 貔貅 (Píxiū), a mythical beast that only takes in and never loses — symbolizing memory that, once stored, is never forgotten.

**Rust-powered · Local-first · End-to-end encrypted · MCP-native · Single binary**

---

## Why Besure AI Context?

You work on multiple projects. You switch between tasks. Every time you switch, you lose context — what you were doing, what you decided, what you learned. AI agents have the same problem.

**Besure AI Context fixes this:**

| Problem | Solution |
|---------|----------|
| 🔀 **Context loss** when switching projects | Git-branch-like context isolation — work on one, switch instantly |
| 🤖 **AI agents can't remember** across sessions | Native MCP Server (23 tools) — Claude, Cursor, OpenClaw can store & retrieve context |
| 🔐 **Multiple agents, no isolation** | Multi-vault architecture — each agent gets its own physically isolated vault |
| ☁️ **Cloud dependency & privacy concerns** | 100% local — SQLite + Markdown, zero cloud required |
| 🔓 **Data security** | AES-256-GCM + Argon2id encryption — keys never touch disk |
| 📦 **Setup complexity** | Single binary, zero runtime dependencies — `curl | bash` and you're done |

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

# Export a context to share
besure export "My Project" -o project-context.md
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

## Connect to AI Agents (MCP)

Besure AI Context includes a native MCP (Model Context Protocol) server. Any MCP-compatible AI tool can store and retrieve context:

### Claude Desktop / OpenClaw / Cursor

Add to your MCP config:

```json
{
  "mcpServers": {
    "besure": {
      "command": "besure",
      "args": ["mcp"]
    }
  }
}
```

Now your AI agent can:
- **List contexts** → see all your projects
- **Add entries** → record decisions and progress
- **Search memory** → find relevant past context
- **Query with filters** → time/type/keyword/resolved filters (V0.4)
- **Resolve entries** → mark tasks as done
- **Append to entries** → supplement existing records
- **View stats** → overview by context/type/status
- **Multi-vault** → isolated vaults per agent, shared vault for collaboration (V0.5)
- **Create contexts** → start new project memory
- **Export & share** → hand off context to teammates

---

## Web Dashboard

```bash
besure serve --port 7788
# → Open http://localhost:7788
```

A built-in web UI for browsing contexts, viewing timelines, and managing entries. Password-protected with your master password.

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
besure search <query>             Full-text search (--semantic for vector)
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

# === Closure (V3) ===
besure link <id> --to <id>        Link entries (caused_by/supersedes/related_to)
besure expire <id>                Mark entry as expired
besure supersede <old> <new>      Mark old entry superseded by new
besure recall                     Recall entries needing attention
besure config set/get/list        Project-level config

# === Server ===
besure serve [--port 7788]        Start web dashboard + REST API
besure mcp                        Start MCP server (stdio, 23 tools)
besure export <context>           Export to Markdown
```

---

## Architecture

```
┌──────────────────────────────────────────┐
│          Besure AI Context                │
│                                          │
│  ┌────────────────────────────────────┐  │
│  │         Interface Layer            │  │
│  │  CLI · MCP Server · REST API · Web │  │
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
| **AI Agent integration** | Native (MCP) | None | Limited | API only |
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
| **V3** | ✅ Done | Closure engine: entry links, expiry, supersede, recall, project config (16 MCP tools) |
| **V0.4** | ✅ Done | Unified query (time/type/keyword/resolved filters), resolve, append, stats (20 MCP tools) |
| **V0.5** | ✅ Done | Multi-vault architecture: physical isolation per agent, global view, shared vault (23 MCP tools) |
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
