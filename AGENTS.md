# AGENTS.md

## Cursor Cloud specific instructions

Besure AI Context is a **pure-Rust, single-binary** project. The primary deliverable is the
`besure` CLI (crate `besure`, binary `src/main.rs`), which bundles four interfaces from one
binary: CLI, MCP server (`besure mcp`), REST API + Web Dashboard (`besure serve`). A secondary,
still-planned **Tauri desktop app** lives in `src-tauri/`.

### Toolchain / environment (already baked into the VM snapshot)

- Requires **Rust stable ≥ 1.85** (the dependency tree pins `idna_adapter` which needs the
  `edition2024` Cargo feature). The snapshot's default toolchain is set to `stable` (1.97+).
  If a future VM ever reverts to an older Rust, run `rustup default stable`.
- The default `cc` on this VM is **clang**, and it selects the **gcc-14** toolchain for the final
  link. Several dependencies (`fastembed` → `hf-hub` → `native-tls` → `openssl-sys`, plus C++
  crates like `onig_sys`/`esaxx_rs`) need OpenSSL and `libstdc++`. The snapshot already has
  `pkg-config`, `libssl-dev`, and `libstdc++-14-dev` installed. Without `libstdc++-14-dev` the
  final link fails with `unable to find library -lstdc++` (clang picks gcc-14, whose libstdc++
  dev files are otherwise missing).

### Build / lint / test / run

- Build (dev): `cargo build` — first build is heavy (~5-10 min, ~290 MB debug binary).
- Test: `cargo test` — 61 unit/integration tests, all passing, ~15s.
- Lint: `cargo clippy` and `cargo fmt --check` (note: `cargo fmt --check` currently reports
  pre-existing formatting diffs in the repo; that is expected, not something to "fix").
- Run the Web Dashboard + REST API: `cargo run -- serve --port 7788` (or
  `./target/debug/besure serve --port 7788`), then open `http://localhost:7788`.
- Run the MCP server: `besure mcp` (stdio JSON-RPC).

### Runtime caveats (non-obvious)

- The dashboard/CLI needs an initialized vault first: `besure init` (unencrypted) or
  `besure init --encrypt`. Vault data lives in `~/.besure`.
- Dashboard auth: for an **unencrypted** vault the dashboard accepts *any* password unless you
  set `BESURE_DASHBOARD_PASSWORD` (it takes priority over vault auth). For local testing:
  `BESURE_DASHBOARD_PASSWORD=demo123 besure serve --port 7788`. REST endpoints under `/api/*`
  require the token returned by `POST /api/auth` (send it as `Authorization: Bearer <token>`).
- Semantic search (`besure index`, `search --semantic`) downloads the local fastembed model
  (`bge-small-zh-v1.5`, ~100 MB) to `~/.cache/huggingface` on first use — needs network once,
  then works offline. It degrades gracefully if unavailable; `besure add` is never blocked.
- Auto-tagging on `besure add` calls an external LLM only if configured via `besure appconfig`
  (`llm.*`). With no key it degrades gracefully (no tags), so it is not required for setup.

### Tauri desktop app (`src-tauri/`) — secondary / out of scope for headless cloud

This wraps the dashboard in a native window and depends on GTK/WebKit system libraries plus a
display server, so it cannot run in the headless cloud VM. It is not needed to develop or test
the primary product. Build it only on a desktop with the Tauri prerequisites installed.
