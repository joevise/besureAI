# 貔貅记忆 Besure AI

### Context Switch Memory System — 通用多上下文管理系统

> 貔貅，只进不出，象征记忆一旦存入，永不丢失。

**本地部署 · CLI 优先 · 可 MCP · 可 Skill · 可插件调用**

---

## 这是什么

为 AI Agent 和人类设计的**本地优先多上下文记忆系统**。

让用户在任意数量的任务/项目之间**秒级切换、引用、交接**，每个上下文完整隔离，又可跨上下文互引。

## 核心能力

- 🗂️ **多上下文管理** — 创建/切换/归档，像 git branch 一样管理任务上下文
- 🔒 **上下文隔离** — 做任务A时只加载A，专注无噪音
- 🔗 **跨上下文引用** — 任务之间可互引成果、决策、产出
- 🔍 **语义搜索** — 内置向量库，自然语言秒搜历史记录
- 📦 **一键打包** — 上下文导出为 .md/.zip，同事接手零摩擦
- 🤖 **Agent 原生** — MCP Server + Skill + REST API，任意 AI 工具可接入
- 💾 **本地优先** — SQLite + Markdown，数据在你手里，离线可用

## 快速开始

```bash
# 安装（开发阶段）
pip install -e .

# 初始化
besure init

# 创建第一个上下文
besure create "我的项目"

# 切换
besure switch ctx_我的项目

# 记录进展
besure add "完成了架构设计"

# 查看所有上下文
besure list

# 语义搜索
besure search "架构设计"

# 导出分享
besure export ctx_我的项目
```

## 接入方式

| 方式 | 场景 |
|------|------|
| **CLI** | 核心交互，开发者日常使用 |
| **MCP Server** | Claude / OpenClaw / Cursor 等 AI 工具原生接入 |
| **Skill** | OpenClaw agent 对话中自然调用 |
| **REST API** | 第三方系统集成 |
| **Plugin** | VS Code / 浏览器 / 桌面（规划中） |

## 技术栈

**Rust 核心**（ring · argon2 · rusqlite · clap · axum）+ **Python AI 引擎**（ChromaDB · embedding · LLM）+ **Tauri 桌面 APP**（React · Tailwind）

生产级安全性 · 内存安全 · 端到端加密 · 跨平台单二进制

## 设计文档

完整设计文档见 [DESIGN.md](./DESIGN.md)

## 项目状态

🚧 **设计阶段** — 架构定稿（Rust + Python + Tauri），MVP 开发即将启动

## License

MIT

---

*貔貅记忆 — 只进不出，记忆永存。*
