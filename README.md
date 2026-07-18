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
| 🤖 **AI agents can't remember** across sessions | Native MCP Server — Claude, Cursor, OpenClaw can store & retrieve context |
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

# Export a context to share
besure export "My Project" -o project-context.md
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
besure init --encrypt         Initialize vault with encryption
besure create <title>         Create a new context
besure switch <query>         Switch context (fuzzy match)
besure add <content>          Add an entry (--type: milestone/decision/progress/blocker/note)
besure list                   List all contexts
besure log [context]          View timeline
besure search <query>         Full-text search (add --semantic for vector search)
besure absorb [--auto]        Extract entries from conversation text (stdin or --from file)
besure export <context>       Export to Markdown
besure serve [--port 7788]    Start web dashboard + REST API
besure mcp                    Start MCP server (stdio)
besure unlock                 Unlock vault
besure lock                   Lock vault
besure status                 Show vault status
besure config set <k> <v>     Configure embedding/LLM API
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
| **V1** | ✅ Done | Vector search, MCP server, Absorb, REST API |
| **V2** | ✅ Done | Web Dashboard with auth, semantic search |
| **V3** | 🚧 Next | Tauri desktop app, cross-device sync |
| **V4** | 📋 Planned | Team collaboration, plugin SDK |
| **V5** | 📋 Planned | VS Code extension, browser extension |

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
