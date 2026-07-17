# 貔貅记忆 Besure AI

### Context Switch Memory System — 通用多上下文管理系统

> 貔貅，只进不出，象征记忆一旦存入，永不丢失。

**本地部署 · CLI 优先 · 可 MCP · 可 Skill · 可插件调用**

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

## 十、技术栈

| 层 | 选择 | 理由 |
|----|------|------|
| **核心语言** | Python 3.11+ | 生态成熟，团队熟悉 |
| **CLI 框架** | Typer (Click) | 标准选择，输出美观，自动补全 |
| **后端** | FastAPI | 轻量，异步，团队熟悉 |
| **存储** | SQLite（元数据） | 零配置，单文件，可移植 |
| **文件格式** | Markdown + JSON frontmatter | 人可读，Git友好，可diff |
| **向量库** | ChromaDB（嵌入式模式） | 无需独立进程，直接嵌入 |
| **Embedding** | BGE-small-zh / text-embedding-3-small | 中文友好 / 兼容性好 |
| **LLM 提取** | 可选，支持 MiniMax/OpenAI/本地模型 | 自动对话→结构化提取 |
| **打包** | pip install + Docker + PyInstaller | 灵活部署 |
| **Web UI** | Alpine.js + Tailwind（可选） | 极轻量，不需要构建工具 |

---

## 十一、部署模式

### 1. 纯本地（默认）

```bash
pip install besure-ai
besure init
# 开始用
# 数据全部在 ~/.besure/ → 可 Git 管理
```

### 2. 本地服务 + 远程访问

```bash
besure serve --port 7788
# 起一个 FastAPI 服务
# Web UI: localhost:7788
# REST API: localhost:7788/api
# MCP: localhost:7788/mcp
```

### 3. 云端部署（团队协作）

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

| 阶段 | 交付物 | 时间 |
|------|--------|------|
| **MVP** | CLI 核心（init/create/switch/add/list/search/export）+ SQLite + Markdown | 2天 |
| **V1** | + ChromaDB 向量检索 + MCP Server + 导出/导入 | +3天 |
| **V2** | + 自动对话提取（接 LLM）+ REST API + 简易 Web UI | +5天 |
| **V3** | + 云端部署 + 多用户 + Docker + 插件 SDK | +1-2周 |
| **V4** | + VS Code 插件 + 浏览器插件 + 桌面客户端 | 后续 |

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

## 十五、项目信息

- **GitHub**: joevise/besureAI
- **本地路径**: `/home/elttilz/joevise-projects/besure-ai/`
- **创建时间**: 2026-07-17
- **作者**: 刘正阳（大Joe）
- **许可证**: MIT（核心）

---

*貔貅记忆 — 只进不出，记忆永存。*
