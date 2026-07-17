# 貔貅记忆 Besure AI

### Context Switch Memory System — 通用多上下文管理系统

> 貔貅，只进不出，象征记忆一旦存入，永不丢失。

**Rust 引擎 · 本地部署 · 桌面 APP · 端到端加密 · 可 MCP · 可 Skill · 可插件调用**

---

## 一、产品定位

为 AI Agent 和人类设计的**本地优先多上下文记忆系统**。

让用户在任意数量的任务/项目之间**秒级切换、引用、交接**，每个上下文完整隔离，又可跨上下文互引。

### 核心差异

| 维度 | Obsidian | Notion | Besure AI |
|------|----------|--------|-----------|
| 定位 | 本地笔记 | 云端协作 | Agent-first 上下文管理 |
| 检索 | 文件名/全文 | 全文+数据库 | 语义向量检索 |
| Agent 集成 | 无 | 有限 | 原生（MCP/Skill/API） |
| 上下文隔离 | 手动管理 | 工作区 | 自动切换+引用 |
| 可打包分享 | 手动导出 | 分享链接 | 一键导出 .md/.zip |
| 部署模式 | 纯本地 | 纯云端 | 本地优先 + 可云端 |

---

## 二、核心痛点

| 痛点 | 场景 |
|------|------|
| **上下文丢失** | 做完任务A切到任务B，再回来A已经不记得了 |
| **信息淹没** | 所有项目的上下文堆在一起，噪音太大 |
| **无法跨项目引用** | 任务A需要用到任务B的成果，没有结构化的引用机制 |
| **交接困难** | 想把某个项目的完整上下文给同事，只能口头或截图 |
| **AI 记忆不可靠** | 靠 LLM 自己"记得"不现实，靠手动记笔记太碎片 |

---

## 三、设计理念

### 1. 本地优先（Local-First）

- 数据在用户自己的机器上，SQLite + Markdown 文件
- 不依赖任何云服务即可完整运行
- 隐私可控，离线可用

### 2. CLI 优先（Developer-First）

- 核心操作全通过命令行完成，快、可脚本化、可管道
- Web UI 是可选的增强，不是必需品
- 像 git 一样自然

### 3. 多接入模式

```
                    ┌──────────────┐
                    │  Besure AI   │
                    │   Engine     │
                    └──────┬───────┘
                           │
          ┌────────┬───────┼───────┬────────┐
          ▼        ▼       ▼       ▼        ▼
       CLI       MCP    Skill   Plugin    REST API
    (核心交互)  (Agent  (OpenClaw (IDE/    (第三方
                原生)   集成)   浏览器)   接入)
```

### 4. 上下文隔离 + 可互引

- 加载上下文A时，只看A，其他全部不加载
- 但可以随时引用任意上下文的片段进来
- 引用是"指针"，不是"复制"——源更新时引用可感知

---

## 四、系统架构

```
┌──────────────────────────────────────────────────────┐
│                   貔貅记忆 Besure AI                   │
│                                                      │
│  ┌─────────────────────────────────────────────────┐ │
│  │              接入层 (Interface Layer)             │ │
│  │  CLI │ MCP Server │ Skill API │ Plugin │ REST   │ │
│  └──────────────────────┬──────────────────────────┘ │
│                         │                            │
│  ┌──────────────────────▼──────────────────────────┐ │
│  │            引擎层 (Context Engine)                │ │
│  │                                                 │ │
│  │  ┌──────────┐ ┌──────────┐ ┌─────────────────┐ │ │
│  │  │上下文管理 │ │ 引用引擎  │ │ 自动提取(可选LLM)│ │ │
│  │  │创建/切换  │ │ 跨上下文  │ │ 对话→结构化记录  │ │ │
│  │  │归档/恢复  │ │ 语义检索  │ │                 │ │ │
│  │  └──────────┘ └──────────┘ └─────────────────┘ │ │
│  └──────────────────────┬──────────────────────────┘ │
│                         │                            │
│  ┌──────────────────────▼──────────────────────────┐ │
│  │             存储层 (Storage Layer)               │ │
│  │                                                 │ │
│  │  ┌──────────┐ ┌──────────┐ ┌─────────────────┐ │ │
│  │  │ SQLite   │ │ Markdown │ │  ChromaDB       │ │ │
│  │  │ (元数据  │ │ (人可读  │ │  (向量检索      │ │ │
│  │  │  +索引)  │ │  +Git)   │ │   语义搜索)     │ │ │
│  │  └──────────┘ └──────────┘ └─────────────────┘ │ │
│  └─────────────────────────────────────────────────┘ │
│                                                      │
│  ┌─────────────────────────────────────────────────┐ │
│  │            同步层 (Sync Layer — 可选)            │ │
│  │  云端同步 │ 多端协作 │ 团队分享 │ 版本历史       │ │
│  └─────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

---

## 五、数据模型

### 存储结构

```
~/.besure/
├── besure.db                        # SQLite 主数据库（元数据+索引+关系）
├── vault/                           # 人可读的 Markdown 文件
│   ├── _index.md                    # 总索引（自动生成）
│   │
│   ├── ctx_brand2context/           # 每个上下文一个目录
│   │   ├── CONTEXT.md               # 完整上下文（人可读 + 可编辑）
│   │   ├── meta.json                # 元数据
│   │   ├── entries/                 # 每次进展记录
│   │   │   ├── 001-init.md
│   │   │   ├── 002-schema-v03.md
│   │   │   └── 003-social-crawler.md
│   │   ├── refs/                    # 引用了哪些其他上下文
│   │   │   └── from-link2context.json
│   │   └── assets/                  # 附带的文件/截图等
│   │
│   ├── ctx_aiguide/
│   │   ├── CONTEXT.md
│   │   ├── meta.json
│   │   └── entries/
│   │
│   └── ctx_quant_report/
│       ├── CONTEXT.md
│       ├── meta.json
│       └── entries/
│
└── chroma/                          # ChromaDB 向量数据（自动索引）
```

### 核心数据结构

#### meta.json

```json
{
  "id": "ctx_brand2context",
  "title": "Brand2Context — 品牌知识库生成器",
  "status": "active",
  "created": "2026-03-29",
  "updated": "2026-07-17",
  "tags": ["python", "fastapi", "mcp", "saas"],
  "progress": 60,
  "related": ["ctx_link2context", "ctx_aiguide"],
  "summary": "输入品牌官网URL → 输出AI可调用的结构化品牌知识库",
  "current_milestone": "社交媒体集成",
  "next_steps": ["登录页二维码方案", "用户系统", "导出功能"],
  "shareable": true
}
```

#### CONTEXT.md（完整上下文，人可读）

```markdown
# Brand2Context — 品牌知识库生成器

## 背景
输入品牌官网URL → 输出AI可调用的结构化品牌知识库MCP Server/API

## 当前状态
- Schema v0.3（11维度）
- 已上线：67.209.190.54:3002/8004/8005
- 社交媒体集成进行中

## 关键决策记录
- [2026-03-29] 选择 FastAPI + Next.js（而非 Django）
- [2026-03-31] MediaCrawler 作为社交数据源
- [2026-07-15] 决定用 Playwright 截图二维码方案

## 进展时间线
- 03-29: 项目启动，Schema v0.2
- 03-30: 核心抓取+LLM结构化
- 04-01: Web界面+MCP端点
- 04-05: Schema v0.3 + 社交集成启动
- 07-15: 二维码截图方案确定

## 引用了其他上下文
- ← link2context: 抓取模块设计
- ← quant_report: 数据管道架构经验

## 被引用
- → aiguide: 品牌知识库schema参考
```

#### Entry（进展记录）

```markdown
---
id: 003-social-crawler
date: 2026-03-31
type: milestone
tags: [social, mediacrawler]
refs: [ctx_link2context/001-init]
---

## 社交媒体爬虫集成完成

MediaCrawler 测试通过（微博/小红书/抖音），social_api.py 独立服务端口8006。

### 决策
- 用 Playwright 截图二维码，不在后端扫码
- systemd 管理 Xvfb + x11vnc

### 遇到的坑
- Docker 内 Playwright 需要额外依赖
- 三平台都需扫码登录，不能无人值守

### 产出文件
- scripts/social_crawler.py
- scripts/social_api.py
```

### Entry 类型

| type | 含义 | 示例 |
|------|------|------|
| `init` | 上下文初始化 | 项目启动、任务定义 |
| `milestone` | 里程碑 | 阶段性成果完成 |
| `decision` | 关键决策 | 技术选型、方案确定 |
| `progress` | 日常进展 | 完成了某个功能 |
| `blocker` | 遇到阻碍 | 报错、依赖缺失 |
| `note` | 备注 | 参考链接、想法 |
| `ref` | 引用记录 | 从其他上下文引用了什么 |

---

## 六、CLI 设计

### 命令总览

```bash
# === 基础操作 ===
besure init                          # 初始化本地 vault
besure list                          # 列出所有上下文（索引总览）
besure list --status active          # 过滤状态
besure list --tag python             # 按标签过滤

# === 创建 & 切换 ===
besure create "Brand2Context"        # 创建新上下文
besure create "分析AI Embryo竞品" --tag research --tag competitor

besure switch ctx_brand2context      # 切换到某上下文
besure switch brand2context          # 支持模糊匹配
besure current                       # 查看当前激活的上下文

# === 进展记录 ===
besure add "完成了Schema v0.3设计"   # 追加一条进展
besure add --type decision "决定用Playwright截图方案"
besure add --type blocker "Docker内Playwright缺依赖"
besure log                           # 查看当前上下文的完整时间线
besure log ctx_aiguide               # 查看指定上下文的时间线

# === 搜索 & 引用 ===
besure search "抓取模块"             # 语义搜索所有上下文
besure search "用户调研" --context ctx_aiguide
besure search --tag mcp              # 按标签搜索

besure ref ctx_link2context          # 引用另一个上下文的摘要
besure ref ctx_link2context --entry 002-schema  # 引用特定进展
besure refs                          # 查看当前上下文的所有引用关系

# === 导出 & 分享 ===
besure export ctx_brand2context      # 导出为单个 .md 文件
besure export ctx_brand2context --format zip   # 导出完整目录
besure export ctx_brand2context --share        # 生成分享包（含元数据）

besure import brand2ctx-export.md    # 导入别人分享的上下文
besure import team-share.zip         # 导入完整上下文包

# === 状态管理 ===
besure status ctx_brand2context      # 查看某上下文状态详情
besure status                        # 全局状态总览
besure pause ctx_brand2context       # 标记为暂停
besure complete ctx_aiguide          # 标记为完成
besure archive ctx_old_project       # 归档

# === 自动提取（可选，需LLM） ===
besure absorb                        # 从 stdin 读取对话文本，自动提取进展
besure absorb --from chat-log.md     # 从文件读取
besure absorb --auto                 # 自动关联到当前上下文

# === 服务模式 ===
besure serve                         # 启动 REST API + Web UI
besure serve --port 7788             # 指定端口
besure mcp                           # 启动 MCP Server (stdio)

# === 配置 ===
besure config set vault_path ~/.besure/vault
besure config set llm.provider minimax
besure config set llm.api_key sk-xxx
besure config set embedding.model bge-small-zh
```

---

## 七、MCP Server 接入

貔貅记忆内置 MCP Server，任何支持 MCP 的 AI 工具（Claude、OpenClaw、Cursor...）都能直接调用：

### 配置方式

```json
{
  "mcpServers": {
    "besure": {
      "command": "besure",
      "args": ["mcp"],
      "env": {
        "BESURE_VAULT": "~/.besure/vault"
      }
    }
  }
}
```

### 暴露的 MCP Tools

| Tool | 参数 | 说明 |
|------|------|------|
| `besure_list_contexts` | `status?`, `tag?` | 列出所有上下文 |
| `besure_get_context` | `id` | 加载完整上下文 |
| `besure_get_status` | `id` | 查看上下文状态摘要 |
| `besure_add_entry` | `id`, `content`, `type?`, `tags?` | 追加进展 |
| `besure_search` | `query`, `context_id?` | 语义搜索 |
| `besure_reference` | `from_id`, `to_id`, `entry_id?` | 创建引用 |
| `besure_get_refs` | `id` | 查看引用关系图 |
| `besure_export` | `id`, `format?` | 导出上下文 |
| `besure_import` | `path` | 导入上下文 |
| `besure_create` | `title`, `tags?`, `summary?` | 创建新上下文 |

---

## 八、OpenClaw Skill 接入

作为轻量 Skill 集成，让 OpenClaw agent 在对话中自然使用：

### Skill 配置

```yaml
# ~/.openclaw/workspaces/<user>/skills/besure/SKILL.md
name: besure
description: "貔貅记忆 — 多上下文管理。切换/记录/搜索/引用项目上下文。"
```

### 使用示例

```
用户: 切到 Brand2Context
Agent: → besure switch ctx_brand2context
       → 加载完整上下文返回
       → "已切换到 Brand2Context。上次进度：社交媒体集成，二维码方案已确定。下一步：登录页开发"

用户: 记一下，刚完成了 Playwright 截图模块
Agent: → besure add "完成了Playwright截图模块" --type progress
       → "已记录到 Brand2Context"

用户: link2context 上次那个抓取的坑是什么来着？
Agent: → besure search "抓取 坑" --context ctx_link2context
       → 返回相关 entry
       → "当时 Docker 内 Playwright 缺少系统依赖，后来额外安装了..."

用户: 把 Brand2Context 的上下文打包给我同事
Agent: → besure export ctx_brand2context --format zip
       → "已导出到 ~/.besure/exports/ctx_brand2context.zip"
```

---

## 九、插件接入（IDE / 浏览器 / 桌面）

| 插件形态 | 场景 | 优先级 |
|---------|------|--------|
| **VS Code 插件** | 侧边栏显示上下文列表，写代码时自动加载项目上下文 | P2 |
| **浏览器插件** | 网页选中文本 → 右键存入某上下文 | P3 |
| **Raycast/Alfred** | 快捷键搜上下文、切换、追加进展 | P3 |
| **桌面通知** | 上下文更新/引用提醒 | P4 |

---

## 十、技术栈与系统架构（Rust + Python 生产级）

### 设计原则：生产级安全性与性能，从一开始就上 Rust

核心引擎用 **Rust** 实现加密、文件 I/O、SQLite 操作（性能最优先 + 内存安全），
AI/向量相关能力用 **Python sidecar** 实现（生态无可替代）。
终态打包为 **桌面 APP（Tauri）**，用户双击即用，无需安装任何依赖。

### 三层架构

```
┌───────────────────────────────────────────────────┐
│            Besure AI 桌面 APP (Tauri)              │
│                                                   │
│  ┌─────────────────────────────────────────────┐  │
│  │          前端 UI (React + Tailwind)          │  │
│  │   上下文管理 / 搜索 / 时间线 / 引用图 / 设置  │  │
│  └───────────────────┬─────────────────────────┘  │
│                      │ Tauri IPC (命令调用)        │
│  ┌───────────────────▼─────────────────────────┐  │
│  │          Rust 核心层 (Tauri 后端)             │  │
│  │                                             │  │
│  │  • AES-256-GCM 加密/解密（ring crate）       │  │
│  │  • Argon2id 密钥派生（argon2 crate）         │  │
│  │  • SQLite 操作（rusqlite）                   │  │
│  │  • 文件 I/O + vault 管理                     │  │
│  │  • CLI 引擎（clap）                          │  │
│  │  • MCP Server（原生 Rust 实现）              │  │
│  │  • REST API（axum）                          │  │
│  └───────────────────┬─────────────────────────┘  │
│                      │ sidecar (子进程)            │
│  ┌───────────────────▼─────────────────────────┐  │
│  │      Python AI 引擎 (PyInstaller 打包)       │  │
│  │                                             │  │
│  │  • ChromaDB（向量存储 + 语义检索）           │  │
│  │  • Embedding 生成（BGE / OpenAI）            │  │
│  │  • LLM 自动提取（MiniMax / OpenAI / 本地）   │  │
│  │  • 通过 stdin/stdout JSON 协议与 Rust 通信   │  │
│  └─────────────────────────────────────────────┘  │
│                                                   │
│  打包发布：                                        │
│  ├── macOS:  .dmg / .app (Universal Binary)       │
│  ├── Windows: .exe / .msi (x64 + ARM64)           │
│  └── Linux:  .deb / .AppImage / .rpm              │
│  用户不需要安装 Python / Rust — 全部内置            │
└───────────────────────────────────────────────────┘
```

### 为什么 Rust + Python 双层

| 维度 | Rust 层 | Python 层 |
|------|---------|----------|
| **职责** | 加密、存储、文件I/O、CLI、MCP、API | 向量检索、Embedding、LLM 调用 |
| **性能** | 极快（原生编译，零开销抽象） | 够用（AI 推理瓶颈在模型不在语言） |
| **安全** | 内存安全（无 buffer overflow） | 沙箱 sidecar（即使出问题不影响核心） |
| **包大小** | 极小（静态编译，无运行时） | PyInstaller 打包 ~30MB |
| **生态** | ring/argon2/rusqlite/axum/clap | ChromaDB/transformers/openai |

**核心洞察**：加密和存储是安全命脉，用 Rust 保证不可攻破；AI 能力是功能扩展，用 Python 保证生态丰富。两者通过 JSON 协议 over stdin/stdout 通信，解耦干净。

### 技术栈详情

#### Rust 层（核心引擎）

| 组件 | Crate | 说明 |
|------|-------|------|
| **加密** | `ring` / `aes-gcm` | AES-256-GCM 加密 |
| **密钥派生** | `argon2` | Argon2id（抗GPU暴力破解）|
| **数据库** | `rusqlite` | SQLite 绑定（编译进二进制）|
| **CLI** | `clap` | 命令行解析，自动补全，帮助生成 |
| **HTTP** | `axum` | REST API 服务器 |
| **序列化** | `serde` / `serde_json` | JSON 序列化/反序列化 |
| **文件锁** | `fs2` | 文件锁（防多实例冲突）|
| **Tauri** | `tauri` | 桌面 APP 壳（终态）|

#### Python 层（AI 引擎 sidecar）

| 组件 | 包 | 说明 |
|------|-----|------|
| **向量库** | `chromadb` | 嵌入式向量存储 |
| **Embedding** | `sentence-transformers` / `openai` | 本地或远程 embedding |
| **LLM** | `openai` / `httpx` | MiniMax/OpenAI/本地模型调用 |
| **打包** | `pyinstaller` | 打包为单二进制（sidecar） |

#### 前端（桌面 APP UI）

| 组件 | 选择 | 说明 |
|------|------|------|
| **框架** | React 18 | 生态最大，组件丰富 |
| **样式** | Tailwind CSS | 快速开发，一致性好 |
| **状态** | Zustand / Jotai | 轻量状态管理 |
| **构建** | Vite | 极速 HMR |

### 交付形态

```
Besure AI
├── besure-core (Rust crate)     ← 核心引擎（加密+存储+CLI+API+MCP）
├── besure-ai (Python pkg)       ← AI 引擎 sidecar（向量+LLM）
├── besure-app (Tauri APP)       ← 桌面应用（前端+壳）
└── besure-server (Docker)       ← 云端协作版（可选）
```

| 交付物 | 用户怎么拿到 | 目标用户 |
|--------|------------|--------|
| **桌面 APP** | GitHub Releases 下载 .dmg/.exe/.deb | 所有人（终态主力） |
| **CLI 二进制** | `brew install besure` / `cargo install besure` | 开发者 |
| **MCP Server** | 内置于 APP / 独立运行 | AI Agent 用户 |
| **REST API** | `besure serve` | 第三方集成 |
| **云端版** | SaaS 订阅 | 团队/企业 |

---

## 十一、部署模式

### 1. 桌面 APP（默认，终态主力）

```bash
# 下载 BesureAI.dmg (macOS) / BesureAI.exe (Windows) / BesureAI.AppImage (Linux)
# 双击安装，打开即用
# 数据全部在 ~/.besure/ → 加密存储
# 内置 Python sidecar（用户无感）
```

### 2. CLI 模式（开发者）

```bash
# macOS
brew install besure-ai
# 或 cargo install
cargo install besure-ai

# 或直接下载预编译二进制
curl -L https://github.com/joevise/besureAI/releases/latest/download/besure-linux-x64 -o /usr/local/bin/besure

besure init
# 开始用
```

### 3. 本地服务 + 远程访问

```bash
besure serve --port 7788
# Rust axum 服务器
# REST API: localhost:7788/api
# MCP: localhost:7788/mcp
# Web UI: localhost:7788（V2 阶段）
```

### 4. 云端部署（团队协作）

```bash
docker run -d \
  -p 7788:7788 \
  -v ~/.besure:/data \
  -e BESURE_MODE=server \
  -e BESURE_MULTI_USER=true \
  besure-ai/server:latest

# 多用户支持
# 上下文共享/协作
# 团队交接场景
```

---

## 十二、与其他系统的关系

```
┌─────────────────────────────────────────────────────┐
│                  用户的工作流                         │
│                                                     │
│  ┌──────────┐    ┌──────────┐   ┌───────────┐      │
│  │ OpenClaw │◄──►│ Besure AI│◄──►│  GitHub   │      │
│  │ (Agent)  │    │ (记忆)   │   │  (代码)   │      │
│  └──────────┘    └────┬─────┘   └───────────┘      │
│                       │                             │
│              ┌────────▼────────┐                    │
│              │   CoreMemory    │                    │
│              │ (公共长期记忆)   │                    │
│              └─────────────────┘                    │
│                                                     │
│  Besure: 任务级上下文（多、细、可切换、可互引）        │
│  CoreMemory: 跨任务公共知识（少、精、始终在）          │
└─────────────────────────────────────────────────────┘
```

**分工**：
- **Besure AI**：任务/项目级上下文，多、细、按需加载、可切换、可互引、可打包
- **CoreMemory**：跨任务公共层（人物、公司、凭据、经验教训），少而精，始终在
- **OpenClaw / 任意 Agent**：Agent 运行时，通过 MCP/Skill/CLI/API 调用 Besure

---

## 十三、开发路线图

| 阶段 | 交付物 | 用户体感 | 时间 |
|------|--------|---------|------|
| **MVP** | Rust 核心引擎：加密+SQLite+CLI（init/create/switch/add/list/export） | `besure init` 开始用，纯 CLI | 1周 |
| **V1** | + Python sidecar：ChromaDB 向量检索 + MCP Server + 导出/导入 | `besure search` 语义搜索可用，AI Agent 可接入 | +1周 |
| **V2** | + 桌面 APP（Tauri）：React UI + 完整可视化 | 双击打开，GUI 操作，所有人可用 | +2-3周 |
| **V3** | + REST API + 自动对话提取（LLM）+ 云端同步 | 服务模式 + AI 自动记录进展 | +2周 |
| **V4** | + 云端版（多用户协作）+ Docker + 插件 SDK | 团队协作 / SaaS | +1月 |
| **V5** | + VS Code 插件 + 浏览器插件 + Raycast | 全生态覆盖 | 后续 |

### MVP 聚焦（第一步只做这些）

```
besure init --encrypt          # 初始化 + 设置主密码
besure create "项目名"         # 创建上下文
besure switch ctx_xxx          # 切换
besure add "进展内容"          # 记录
besure list                    # 列表
besure log                     # 时间线
besure search "关键词"         # 搜索（先全文匹配，V1升向量）
besure export ctx_xxx          # 导出
besure unlock / besure lock    # 解锁/锁定
besure status                  # 状态
```

全部用 Rust 实现，零 Python 依赖。加密、SQLite、CLI 三合一单二进制。

---

## 十四、开源策略

```
貔貅记忆 Besure AI
├── 核心 (MIT) — CLI + 引擎 + 存储 + MCP
├── 云端版 (商业) — 多用户 + 协作 + 团队管理
└── 插件 (各自开源) — VS Code / 浏览器 / Raycast
```

核心开源，建立开发者社区。云端协作版商业化。

---

## 十五、安全架构（端到端加密）

### 威胁模型

| 威胁 | 场景 | 严重度 |
|------|------|--------|
| **物理拿到设备** | 笔记本丢了/被偷，硬盘被挂载到另一台机器 | 🔴 最高 |
| **恶意软件** | 木马程序扫描用户目录，窃取 .besure/ 文件 | 🔴 高 |
| **云同步泄露** | 数据同步到 iCloud/Dropbox 被他人访问 | 🟡 中 |
| **备份泄露** | 系统备份文件被他人恢复 | 🟡 中 |
| **多用户共用机器** | 同台服务器上其他用户读 ~/.besure/ | 🟡 中 |

### 双层加密架构（保险箱模式）

```
┌──────────────────────────────────────────────────┐
│              用户视角                              │
│                                                  │
│  besure unlock  ←→  正常使用  ←→  besure lock     │
│     ↓                    ↑              ↓        │
│  输入主密码         内存中明文操作          加密落盘  │
│                                                  │
├──────────────────────────────────────────────────┤
│              磁盘视角                              │
│                                                  │
│  ~/.besure/                                      │
│  ├── besure.db.enc        ← 加密的 SQLite        │
│  ├── vault/                                      │
│  │   └── *.md.enc         ← 加密的 Markdown     │
│  └── chroma/                                    │
│      └── *.enc            ← 加密的向量数据        │
│                                                  │
│  拿走硬盘？看到的只是一堆 .enc 文件，无法解密      │
└──────────────────────────────────────────────────┘
```

### 核心设计

#### 1. 主密码（Master Password）

```
besure init --encrypt
→ 设置主密码（二次确认）
→ 密码经 Argon2 派生为 256 位加密密钥
→ 密钥永远不落盘，只在内存中

besure unlock
→ 输入密码 → Argon2 派生密钥 → 解密索引 → 正常使用

besure lock   (或 N 分钟无操作自动锁)
→ 密钥从内存清除（主动 overwrite）→ 所有文件恢复加密状态
```

#### 2. 文件级加密（AES-256-GCM）

每个文件独立加密：
- SQLite 数据库整体加密
- 每个 Markdown 文件独立加密
- 文件名也可加密（Paranoia 模式）

#### 3. 密钥派生（Argon2id）

```python
from argon2 import low_level

# 用户密码 → Argon2id → 256位加密密钥
key = argon2.hash_secret_raw(
    secret=password.encode(),
    salt=stored_salt,        # 存在配置里，不加密
    hash_len=32,             # 256 bits
    time_cost=3,             # 迭代次数
    memory_cost=65536,       # 64MB（抗GPU暴力破解）
    parallelism=4,
    type=argon2.low_level.Type.ID  # Argon2id
)
# key 用于 AES-256-GCM 加解密
```

**为什么用 Argon2id**：
- 抗 GPU/ASIC 暴力破解（内存硬函数）
- 2015 年密码哈希竞赛冠军
- 即使敌人拿到加密文件 + 盐，没有密码也破解不了
- 64MB 内存消耗 × 3 次迭代 = 单次尝试成本极高

### 加密后的存储结构

```
~/.besure/
├── .besure.config         # 配置（不含敏感数据，不加密）
│   ├── salt: "随机盐值"          # Argon2 盐（公开无妨）
│   ├── encryption: true          # 是否启用加密
│   ├── auto_lock_minutes: 5      # 自动锁定时间
│   ├── kdf: "argon2id"           # 密钥派生函数
│   └── security_level: "standard" # 安全等级
│
├── besure.db.enc          # 加密的 SQLite（整体加密）
├── vault/
│   ├── ctx_brand2context/
│   │   ├── CONTEXT.md.enc       # 加密的
│   │   ├── meta.json.enc        # 加密的
│   │   └── entries/
│   │       ├── 001.md.enc       # 加密的
│   │       └── 002.md.enc       # 加密的
│   └── ctx_aiguide/
│   └── ...
│
└── chroma/
    └── chroma-data.enc     # 加密的向量数据
```

### 方案选型

| 方案 | 优点 | 缺点 | 推荐度 |
|------|------|------|--------|
| **SQLCipher + 文件加密** | SQLite 原生支持，成熟稳定 | 需要额外依赖 SQLCipher | ⭐⭐⭐⭐ |
| **纯文件级 AES-256-GCM** ✅ | 无额外依赖，Python cryptography 库即可 | 需自己管理加密/解密逻辑 | ⭐⭐⭐⭐⭐ |
| **age 加密（外部工具）** | 最现代，简单 | 外部依赖 | ⭐⭐⭐ |
| **GPG** | 成熟 | 太重，用户体验差 | ⭐⭐ |

**选择：纯文件级 AES-256-GCM**

原因：
- **零外部依赖**：Python `cryptography` 库即可，不需要编译 SQLCipher
- **灵活**：每个 .md 文件独立加密，单文件损坏不影响其他
- **可移植**：加密逻辑纯 Python，任何平台都能跑
- **够安全**：AES-256-GCM 是军用级别，配合 Argon2id 密钥派生，暴力破解不现实

### 关键安全特性

| 特性 | 说明 |
|------|------|
| **密钥永不落盘** | 密码 → Argon2id → 密钥，只存在内存。关机/锁定后消失 |
| **自动锁定** | N分钟无操作自动锁，密钥从内存清除（防恶意软件抓内存） |
| **防暴力破解** | Argon2id（64MB内存消耗 + 3次迭代），GPU 暴力破解成本极高 |
| **文件独立加密** | 每个 .md 独立加密，单文件泄露不影响其他 |
| **导出控制** | `besure export` 时可选择加密或不加密，默认加密 |
| **内存安全清除** | 密钥使用后主动 overwrite 内存（防内存dump） |
| **完整性校验** | AES-256-GCM 自带认证标签，篡改即可检测 |

### 安全等级可选

```bash
besure init
→ Choose security level:
  1. None      (明文存储，最快，开发/测试用)
  2. Standard  (文件加密，主密码保护) ← 默认推荐
  3. Paranoia  (加密 + 文件名混淆 + 内存锁定 + 防侧信道)
```

| 等级 | 加密 | 文件名 | 自动锁定 | 内存保护 |
|------|------|--------|---------|---------|
| None | ❌ | 明文 | ❌ | ❌ |
| Standard | ✅ AES-256-GCM | 明文 | ✅ 5min | ✅ overwrite |
| Paranoia | ✅ AES-256-GCM | 混淆 | ✅ 1min | ✅ overwrite + mlock |

### CLI 安全命令

```bash
# 初始化时启用加密
besure init --encrypt
→ 设置主密码（二次确认）

# 日常使用
besure unlock                    # 解锁（输入密码）
besure lock                      # 立即锁定
besure status                    # 显示 🔒/🔓 状态

# 修改密码
besure password                  # 修改主密码（需旧密码验证）

# 导出（安全分享）
besure export ctx_xxx            # 默认加密导出（需设置分享密码）
besure export ctx_xxx --no-encrypt  # 明文导出（会显示安全警告）

# 自动锁定配置
besure config set auto_lock_minutes 5   # 5分钟无操作自动锁
besure config set auto_lock_on_exit true  # CLI退出即锁
```

### 对 MCP/API 接入的影响

```
外部 Agent → MCP → besure mcp
                     ↓
              检查：已解锁？
              ├── 是 → 正常返回数据
              └── 否 → 返回 "Vault locked. Run 'besure unlock' first."
```

- MCP Server 启动时需要先解锁，之后保持解锁状态直到锁定
- 云端模式可配置**服务级密码**或 **API Token 认证**
- 多用户场景下每个用户有独立 vault + 独立密钥

### 加密实现（Rust 核心层）

加密引擎完全用 Rust 实现，保证内存安全和性能：

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit}};
use argon2::{Argon2, Algorithm, Version, Params};;
use zeroize::Zeroize;

/// 保险箱加密引擎
pub struct VaultCrypto {
    salt: Vec<u8>,
    key: Option<[u8; 32]>,  // 密钥只在内存，Option<None> = 已锁定
}

impl VaultCrypto {
    /// 密码 → Argon2id → 256位密钥
    fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
        let params = Params::new(65536, 4, 3, Some(32))  // 64MB, 4线程, 3次迭代
            .unwrap();
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = [0u8; 32];
        argon2.hash_password_into(password.as_bytes(), salt, &mut key)
            .expect("derive failed");
        key
    }

    /// 解锁：验证密码并加载密钥到内存
    pub fn unlock(&mut self, password: &str) -> bool {
        let key = Self::derive_key(password, &self.salt);
        // 用已知验证文件检查密码正确性
        if self.verify_key(&key) {
            self.key = Some(key);
            true
        } else {
            false
        }
    }

    /// 锁定：从内存安全清除密钥（zeroize）
    pub fn lock(&mut self) {
        if let Some(ref mut key) = self.key {
            key.zeroize();  // Rust zeroize crate：防编译器优化跳过清除
        }
        self.key = None;
    }

    /// 加密文件（AES-256-GCM）
    pub fn encrypt_file(&self, plaintext: &[u8], path: &str) -> Result<()> {
        let key = self.key.expect("vault locked");
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let nonce = Nonce::from_slice(&rand::random::<[u8; 12]>());  // 96-bit
        let ciphertext = cipher.encrypt(nonce, plaintext)?;
        // 写入：nonce (12 bytes) + ciphertext
        let mut file_data = nonce.to_vec();
        file_data.extend_from_slice(&ciphertext);
        std::fs::write(format!("{}.enc", path), file_data)?;
        Ok(())
    }

    /// 解密文件
    pub fn decrypt_file(&self, path: &str) -> Result<Vec<u8>> {
        let key = self.key.expect("vault locked");
        let file_data = std::fs::read(path)?;
        let (nonce_bytes, ciphertext) = file_data.split_at(12);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let plaintext = cipher.decrypt(Nonce::from_slice(nonce_bytes), ciphertext)?;
        Ok(plaintext)
    }
}

impl Drop for VaultCrypto {
    fn drop(&mut self) {
        self.lock();  // 析构时自动清除密钥（防忘记手动 lock）
    }
}
```

**Rust 安全优势**：
- `zeroize` crate：保证内存清除不被编译器优化跳过
- `Drop` trait：对象销毁时自动清除密钥，防忘记手动 lock
- 无 GC 延迟：密钥清除即时生效（Python GC 时机不可控）
- 无 buffer overflow：Rust 所有权系统天然防止内存越界

### 安全设计原则总结

1. **密钥与数据分离** — 密钥永远只在内存，数据在磁盘，两者不同时暴露
2. **每个文件独立加密** — 单点泄露不扩散，完整性可独立校验
3. **密码不存储、不传输** — 只在用户大脑和瞬态内存中
4. **默认安全** — 初始化默认推荐 Standard 加密级别
5. **加密从 MVP 开始** — 后加加密等于重写存储层，不如一开始就做进去

---

## 十六、项目信息

- **GitHub**: joevise/besureAI
- **本地路径**: `/home/elttilz/joevise-projects/besure-ai/`
- **创建时间**: 2026-07-17
- **作者**: 刘正阳（大Joe）
- **许可证**: MIT（核心）

---

*貔貅记忆 — 只进不出，记忆永存。*
