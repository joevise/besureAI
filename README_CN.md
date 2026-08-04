# 🐉 Besure AI Context

### 本地优先的上下文记忆系统 — 为 AI Agent 和人类设计

[English](README.md) | [中文](README_CN.md)

> 貔貅，只进不出，象征记忆一旦存入，永不丢失。

**Rust 引擎 · 本地部署 · 端到端加密 · CLI 原生 · 单二进制**

**当前版本：0.65.0** — 真语义搜索：本地 fastembed（bge-small-zh-v1.5，512 维）完全离线运行——零 API 成本、零 key、隐私安全。`besure index` 建向量索引，`besure add` 自动增量索引。

[English](README.md) | 中文

---

## 为什么需要 Besure AI Context？

你同时在做好几个项目。你不断在任务之间切换。每次切换，上下文就丢了——你在做什么、决定了什么、学到了什么。AI Agent 也有同样的问题。

**Besure AI Context 解决这个问题：**

| 痛点 | 方案 |
|------|------|
| 🔀 切换项目时**上下文丢失** | 类似 git branch 的上下文隔离——专注一个，秒级切换 |
| 🤖 **AI Agent 无法跨会话记忆** | CLI 原生接入——Agent 通过 exec `besure` 命令直接存取上下文 |
| 🔐 **多个 Agent 之间无隔离** | 多 Vault 架构——每个 Agent 拥有物理隔离的独立记忆空间 |
| ☁️ **云依赖和隐私担忧** | 100% 本地——SQLite + Markdown，零云服务 |
| 🔓 **数据安全** | AES-256-GCM + Argon2id 加密——密钥永不落盘 |
| 📦 **部署复杂** | 单二进制，零运行时依赖——`curl | bash` 即装即用 |

---

## 核心概念（V0.58）

Besure AI Context 只有**三个**核心概念：

| 概念 | 说明 |
|------|------|
| **Context（上下文）** | 隔离的记忆空间（类似 git branch），一个项目/任务一个。 |
| **Entry（记录）** | 上下文里的单条记忆。一切都是 entry：进展、决策、里程碑、阻碍、笔记、教训、问题。 |
| **自动标签（Auto Tags）** | 每条 entry 带 1-3 个扁平大类标签，由 Agent 在 `besure add` 时通过 `--tags` 传入。标签是涌现式的：共享 `tag_vocab` 标签库复用语义相同的标签，防止同义词爆炸。 |

> **没有 Config 概念。** V0.58 起不再有独立的 "config" 功能——以前当配置存的东西就是普通 entry，靠自动标签组织和检索。（App 级的 LLM/embedding provider 设置存放在 `~/.besure/appconfig.json`，用 `besure appconfig` 管理。）

### 语义搜索（V0.61）

语义搜索**完全本地**运行：内嵌 [`fastembed`](https://crates.io/crates/fastembed) 引擎 + **bge-small-zh-v1.5** 模型（512 维，中文友好）。无 API、无 key、零成本——完全离线可用，数据不出本机。

- **自动增量索引**：每次 `besure add` 同步把新 entry 向量写入 `vectors.db`（模型不可用时优雅降级，绝不阻塞 add）。
- **存量补建**：`besure index --all` 给所有存量 entry 建向量（已索引的自动跳过；`--rebuild` 强制重建）。
- **搜索**：`besure search "语义描述" --semantic` 按意思找记忆，而不是关键词。REST（`GET /api/search?q=...&semantic=true`，vault 级 `GET /api/vaults/:id/search?...&semantic=true`）、Dashboard（"语义搜索"开关）均已支持。
- 首次运行自动下载模型（~100MB）到 HuggingFace 缓存（`~/.cache/huggingface`），之后本地加载仅需 1-2 秒。

---

## 快速开始

### 安装

```bash
# 一键安装（macOS / Linux）
curl -fsSL https://raw.githubusercontent.com/joevise/besureAI/main/install.sh | bash

# 或通过 cargo
cargo install besure

# 或下载预编译二进制
# → https://github.com/joevise/besureAI/releases
```

安装后 Dashboard 自动获得进程守护（崩溃自动重启、开机自启）：

| 平台 | 守护机制 | 管理方式 |
|------|---------|----------|
| **Linux** | systemd 用户服务 | `systemctl --user {start|stop|status} besure-dashboard` |
| **macOS** | launchd 代理 | `launchctl {load|unload} ~/Library/LaunchAgents/com.besure.context.plist` |
| **Windows** | 启动项 + VBS | 登录自动启动 |

```bash
besure service install     # 安装后台服务
besure service status      # 查看是否在运行
besure service uninstall   # 卸载后台服务
```

### 使用

```bash
# 方式一：一键 Setup（推荐）—— 初始化 + 配置 Agent 自动记忆
besure setup --agent-name "Joey" --agent-type openclaw
# 自动检测 AGENTS.md / CLAUDE.md / .cursorrules 等配置文件并注入强制记忆规则

# 方式二：手动初始化
besure init --encrypt

# 创建第一个上下文
besure create "我的项目" --tag rust --summary "做点酷的东西"

# 记录进展
besure add "完成了认证模块" --type milestone
besure add "决定用 Axum" --type decision
besure add "遇到编译错误，已修" --type blocker

# 列出所有上下文
besure list

# 搜索
besure search "认证"

# 切换上下文（模糊匹配）
besure switch "项目"

# 统一查询（V0.4）
besure query                              # 最近 20 条
besure query --last 7d                     # 最近 7 天
besure query --type decision               # 只要决策
besure query --all --keyword "认证"       # 全上下文 + 关键词

# 标记完成
besure resolve <entry_id>

# 导出上下文（默认加密 .besure）
besure export "我的项目" --password *** -o backup.besure
besure export "我的项目" -o backup.besure        # 交互式输入密码

# 旧版 Markdown 导出
besure export "我的项目" --format md -o project.md

# 导入加密 .besure 文件（entry 按 id 去重）
besure import backup.besure --password ***
```

### 多 Vault：每个 Agent 一个独立记忆空间（V0.5）

每个 AI Agent 拥有物理隔离的独立 vault。Agent 之间默认不可见，共享需显式推送。

```bash
# 通过环境变量配置每个 Agent 的 vault
export BESURE_VAULT=~/.besure/joey          # Joey 的 vault
export BESURE_VAULTS_ALL=true               # 授予全局视角（仅主 Agent）

# 其他 Agent 配自己的 vault，没有全局视角
export BESURE_VAULT=~/.besure/coding-agent  # 编码 Agent 的 vault

# 列出所有 vault（需要全局视角）
besure vaults

# 跨 vault 查询
besure query --all-vaults

# 推送到共享空间
besure share <entry_id>

# 查看共享内容
besure shared
```

---

## 接入 AI Agent（CLI）

Besure AI Context 是 CLI 原生的。任何能执行 shell 命令的 AI Agent（Claude Code、OpenClaw、Cursor、Codex 等）都可以直接运行 `besure` 命令来存取上下文。

### 接入方式

1. **注入强制记忆铁律** —— 把 `skill/SKILL.md` 的铁律加入 Agent 配置文件（AGENTS.md / CLAUDE.md / .cursorrules）。铁律告诉 Agent 什么时候必须记录（完成任务、做决策、commit、部署之后）。
2. **Agent 通过 exec 调用 `besure` 命令** —— 无协议、无额外服务进程，每次调用都是一次独立的 `besure` 命令。

或者一条命令搞定（初始化 + 自动检测 Agent 配置文件并注入铁律）：

```bash
besure setup --agent-name "Joey" --agent-type openclaw
```

AI Agent 可以：
- **列出上下文** → `besure list`，看到所有项目
- **添加记录** → `besure add "..." --type decision --context <id>`（Agent 通过 --tags 传标签）
- **搜索记忆** → `besure search "..."` / `besure search "..." --semantic`
- **统一查询** → `besure query --last 7d --type decision`（V0.4）
- **查看标签库** → `besure tags`（V0.58）
- **标记完成** → `besure resolve <entry_id>`
- **追加内容** → `besure append <entry_id> "..."`
- **统计概览** → `besure stats`（V0.58 起 Stats 以 By Tag 为主）
- **多 Vault** → 每个 Agent 通过 `BESURE_VAULT` 独立隔离，共享需显式推送（V0.5）
- **创建上下文** → `besure create "..."`
- **导出分享** → `besure export` / `besure import` / `besure share`

完整命令列表见下方 [CLI 命令](#cli-命令)，Agent 侧的 skill 文档见 `skill/SKILL.md`。

---

## macOS 桌面 App（V0.64）

基于 **Tauri 2** 打造的原生 macOS 应用——CLI 的全部能力，装进轻量快速的桌面体验：

- **Onboarding 引导** —— 首次启动向导带你完成 vault 创建和 Agent 配置。
- **菜单栏托盘** —— 常驻菜单栏，随时快速访问你的上下文。
- **多 Vault Dashboard** —— 一个窗口浏览、搜索、管理所有 vault 的记录。
- **多 Vault 自动发现（V0.65）** —— 自动发现根 vault 及其所有子目录下的 vault。
- **一键安装 Skill** —— 一键把 Besure skill 装进你的 Agent 工具：Cursor、Codex、OpenClaw、Windsurf、Claude Code、Cline、Hermes、Trae、WorkBuddy、Aider、GitHub Copilot。

从 [Releases](https://github.com/joevise/besureAI/releases) 下载 `.dmg`，或从源码构建：

```bash
./build-macos.sh
```

---

## Web Dashboard

```bash
besure serve --port 7788
# → 浏览器打开 http://localhost:7788
```

内置 Web 界面，浏览上下文、查看时间线、按标签筛选、管理记录。Stats 页已改为 **By Tag**（V0.58）。

**Dashboard 密码安全：**
- vault 加密时，Dashboard 使用主密码登录。
- vault 未加密（或想用独立的 Dashboard 密码）时，启动服务前设置环境变量 `BESURE_DASHBOARD_PASSWORD`——它优先于 vault 认证。
- ⚠️ 如果 vault 未加密**且**未设置 `BESURE_DASHBOARD_PASSWORD`，Dashboard 会接受任意密码（不安全，启动时会打印警告）。只要 Dashboard 暴露范围超出本机，务必二选一设置。

---

## 安全

| 特性 | 详情 |
|------|------|
| **加密** | AES-256-GCM（军用级，带认证标签） |
| **密钥派生** | Argon2id（64MB 内存 / 3 次迭代 / 4 线程——抗 GPU 暴力破解） |
| **密钥存储** | 密钥只存在内存，lock 时 zeroize 清除——永不落盘 |
| **文件级加密** | 每个文件独立加密——单文件泄露不影响其他 |
| **认证** | Dashboard 需主密码——同一密钥既解密数据又授权访问 |
| **无云端** | 零外部网络调用。数据永不离开你的机器。 |

---

## CLI 命令

```
# === Vault ===
besure init --encrypt             初始化（启用加密）
besure unlock                     解锁
besure lock                       锁定
besure status                     查看状态

# === 上下文 ===
besure create <title>             创建上下文
besure switch <query>             切换上下文（模糊匹配）
besure list                       列出所有上下文

# === 记录 ===
besure add <content>              添加记录（--type, --from-file）
besure log [context]              查看时间线
besure search <query>             全文搜索（--semantic 本地语义向量搜索）
besure index [--all]              建语义向量索引（本地 fastembed，离线）
besure absorb [--auto]            从对话提取记录

# === 查询与管理（V0.4）===
besure query                      统一查询（时间/类型/关键词/resolved）
  --last 7d                       最近 N 天
  --from / --to                   日期范围
  --type <t>                      类型过滤（可重复）
  --all                           搜所有上下文
  --keyword <kw>                  关键词
  --unresolved / --resolved       resolved 过滤
  --limit <n>                     返回条数（默认 20）
besure resolve <entry_id>         标记完成
besure append <entry_id> <text>  追加内容到已有记录
besure stats                      统计概览

# === 多 Vault（V0.5）===
besure vaults                     列出所有 vault（需 BESURE_VAULTS_ALL=true）
besure query --all-vaults         跨 vault 查询
besure share <entry_id>           推送到共享 vault
besure share-context <ctx_id>     推送整个上下文
besure shared [--keyword <kw>]    查看共享内容

# === 标签（V0.58）===
besure add <content> --tags a,b,c  Agent 通过 --tags 传 1-3 个大类标签
besure tags                       查看标签库（标签 + 使用次数）
besure retag [--all] [--context <id>]  给存量 entry 补标签

# === App 配置（LLM / embedding provider）===
besure appconfig <key> <value>    设置 app 级配置，如：
                                  llm.provider / llm.api_url / llm.api_key / llm.model
                                  embedding.provider / embedding.api_url / embedding.api_key / embedding.model

# === 闭环（V3）===
besure link <id> --to <id>        关联记录（caused_by/supersedes/related_to）
besure expire <id>                标记过期
besure supersede <old> <new>      标记替代
besure recall                     召回需要注意的记忆

# === 回收站（V0.60）===
besure delete context <id>        软删除 context（含其 entries，移入回收站）
besure delete entry <id>          软删除 entry
besure trash                      查看回收站
besure restore <id>               从回收站恢复（自动识别 context/entry）
besure purge <id>                 永久删除（不可恢复）

# === 服务 ===
besure setup [--agent-name <n>]      一键配置：初始化 + Agent 铁律注入
besure serve [--port 7788]        启动 Web Dashboard + REST API
besure export <context>           导出为加密 .besure（默认）
besure export <context> --format md   导出为 Markdown（旧版）
besure import <file.besure>       导入加密 .besure（按 id 去重）
```

## `besure setup` — 开箱即用

```bash
$ besure setup --agent-name "Joey" --agent-type openclaw

🐉 Besure AI Context — Setup

Step 1: Initialize vault
  ✓ Vault created at ~/.besure/joey

Step 2: Detect Agent configuration files
  ✓ Found: AGENTS.md

Step 3: Inject mandatory recording rules
  ✓ Injected rules into AGENTS.md

✅ Setup complete!
```

支持自动检测的配置文件：

| 文件 | 平台 |
|------|------|
| `AGENTS.md` | OpenClaw / Codex / Hermes / WorkBuddy |
| `.hermes.md` | Hermes Agent |
| `CLAUDE.md` | Claude Code |
| `.cursorrules` | Cursor |
| `.codebuddy/rules.md` | 腾讯 CodeBuddy |

注入用 `<!-- BESURE-AUTO-START/END -->` 标记包裹，幂等执行，重复运行自动更新。

---

## 路线图

| 阶段 | 状态 | 功能 |
|------|------|------|
| **MVP** | ✅ 完成 | 加密引擎、SQLite、CLI、Markdown 文件 |
| **V1-V2** | ✅ 完成 | 向量检索、Absorb、REST API、Web Dashboard + 认证 |
| **V3** | ✅ 完成 | 闭环引擎：关联/过期/替代/召回 |
| **V0.4** | ✅ 完成 | 统一查询（时间/类型/关键词/resolved）、resolve、append、stats |
| **V0.5** | ✅ 完成 | 多 Vault 架构：物理隔离、全局视角、共享 vault |
| **V0.5.5** | ✅ 完成 | Dashboard 多 Agent 视角：侧边栏 Agent 列表、切换数据源 |
| **V0.56** | ✅ 完成 | `besure setup` + 强制记忆铁律：多平台检测、幂等注入 AGENTS.md |
| **V0.58** | ✅ 完成 | 涌现式标签：砍掉 Config 概念，一切归于 entry + 扁平大类标签。Agent 通过 --tags 参数传标签，tag_vocab 标签库复用防同义词爆炸，`besure tags` / `besure retag`，Dashboard Stats 改为 By Tag |
| **V0.59** | ✅ 完成 | 加密导出/导入：`.besure` 自有加密格式（AES-256-GCM + Argon2id，不是 zip——无密码无法被任何工具打开）。`besure export --password` / `besure import --password`，vault-scoped REST 端点，Dashboard Export/Import UI |
| **V0.60** | ✅ 完成 | 回收站机制：Context/Entry 软删除入回收站，可恢复、可永久清除。`besure delete/restore/trash/purge`，Dashboard Trash 视图，所有列表/统计/查询排除已删除项 |
| **V0.61** | ✅ 完成 | 真语义搜索：本地 fastembed + bge-small-zh-v1.5（512 维），完全离线/零成本/零 key。`besure index` 补建索引、add 自动增量索引、`search --semantic`、REST `?semantic=true`、Dashboard 语义搜索开关 |
| **V0.62** | ✅ 完成 | 移除 MCP 层：Agent 通过 CLI（exec `besure` 命令）+ SKILL.md 铁律接入。删除 `besure mcp` 命令 |
| **V0.63** | ✅ 完成 | Agent 驱动标签：Agent 通过 `--tags` 参数传标签，移除 OpenRouter LLM 打标器——不再需要外部 LLM |
| **V0.64** | ✅ 完成 | macOS 桌面 App（Tauri 2）：onboarding 引导流程、菜单栏托盘、多 Vault Dashboard |
| **V0.65** | ✅ 完成 | 多 Vault 自动发现：自动发现根 vault 及其所有子目录下的 vault |
| **下一步** | 📋 计划中 | crates.io 发布、GitHub Actions CI、Product Hunt 上线 |
| **未来** | 📋 计划中 | VS Code 插件、浏览器插件、团队协作 |

---

## 技术栈

100% Rust，单二进制，零外部依赖：

| 组件 | Crate |
|------|-------|
| 加密 | `aes-gcm` + `argon2` + `zeroize` |
| 数据库 | `rusqlite`（SQLite 编译进二进制） |
| CLI | `clap` |
| HTTP | `reqwest`（rustls-tls，不依赖 OpenSSL） |
| REST API | `axum` + `tokio` |
| 序列化 | `serde` + `serde_json` |

---

## 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

MIT — 见 [LICENSE](LICENSE)。

---

*Besure AI Context — 只进不出，记忆永存。🐉*
