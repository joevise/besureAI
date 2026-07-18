## ✨ Features

- 🔒 **AES-256-GCM + Argon2id** — Military-grade encryption, GPU-cracking resistant
- 🗂️ **Context Isolation** — Git-branch-like project switching, zero cross-talk
- 🔍 **Semantic Search** — Built-in vector store with cosine similarity (no ChromaDB needed)
- 🤖 **MCP Native** — Works with Claude, Cursor, OpenClaw out of the box
- 🌐 **Web Dashboard** — Built-in UI with password authentication
- 📝 **Markdown + JSON** — Human-readable files, Git-friendly
- 💾 **100% Local** — SQLite + files, zero cloud dependency
- 🦀 **Pure Rust** — Single binary, zero runtime dependencies
- 🧠 **Auto-Extract** — Absorb progress entries from conversation logs

## 📦 Install

```bash
curl -fsSL https://raw.githubusercontent.com/joevise/besureAI/main/install.sh | bash
```

## 🚀 Quick Start

```bash
besure init --encrypt
besure create "My Project" --tag rust
besure add "Built the core engine" --type milestone
besure search "engine"
```

## 🔗 MCP Integration

```json
{"mcpServers":{"besure":{"command":"besure","args":["mcp"]}}}
```

## 📖 Documentation

- [README (English)](README.md)
- [README (中文)](README_CN.md)
- [Design Document](DESIGN.md)
- [Contributing](CONTRIBUTING.md)
