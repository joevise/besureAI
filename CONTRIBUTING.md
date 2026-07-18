# Contributing to Besure AI Context

Thanks for your interest in contributing! 🐉

## Getting Started

### Prerequisites

- Rust 1.70+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Git

### Build from Source

```bash
git clone https://github.com/joevise/besureAI.git
cd besureAI
cargo build
cargo test
```

### Run the Dashboard

```bash
cargo run -- serve --port 7788
```

## Project Structure

```
src/
├── main.rs              # CLI entry point (clap)
├── crypto/
│   ├── mod.rs
│   └── vault_crypto.rs  # AES-256-GCM + Argon2id encryption engine
├── storage/
│   ├── mod.rs
│   ├── models.rs        # Context, Entry, ContextStatus
│   ├── db.rs            # SQLite CRUD + full-text search
│   └── vault.rs         # Vault management (init/open/unlock/lock)
├── ai/
│   ├── mod.rs
│   ├── embedding.rs     # Embedding API provider (OpenAI/MiniMax/dummy)
│   ├── vector.rs        # Vector store + cosine similarity search
│   ├── absorb.rs        # Auto-extract entries from conversations
│   ├── mcp_server.rs    # MCP Server (stdio JSON-RPC)
│   └── rest_api.rs      # REST API + Dashboard (axum)
├── dashboard.rs         # Embedded HTML dashboard
└── dashboard.html       # Dashboard UI
```

## Development Workflow

1. **Fork & clone** the repo
2. **Create a branch**: `git checkout -b feature/your-feature`
3. **Write code** — follow existing style, keep tests passing
4. **Test**: `cargo test`
5. **Commit**: use clear messages (e.g., `feat: add fuzzy search for tags`)
6. **Push & PR**: open a pull request

### Commit Message Convention

- `feat:` new feature
- `fix:` bug fix
- `docs:` documentation
- `refactor:` code restructuring
- `test:` test additions
- `ci:` CI/build changes

## Areas We Need Help With

- 🌐 **i18n** — Dashboard language switching
- 🖥️ **Tauri desktop app** — wrap in native window
- 🔌 **VS Code extension** — sidebar integration
- 📦 **Package managers** — Homebrew formula, AUR package, Snap
- 📖 **Documentation** — English translation of DESIGN.md, tutorials
- 🧪 **Testing** — integration tests, edge cases, performance benchmarks

## Code Style

- Follow `rustfmt` defaults: `cargo fmt`
- Pass `cargo clippy` without warnings
- Every new function should have a doc comment
- Every new module should have at least one test

## Reporting Issues

- **Bugs**: include OS, Rust version, `besure --version`, and steps to reproduce
- **Feature requests**: describe the use case, not just the solution
- **Security issues**: email directly, don't open a public issue

## License

By contributing, you agree that your contributions will be licensed under the MIT license.

---

*Besure AI Context — Once stored, never lost. 🐉*
