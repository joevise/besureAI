# 🐉 Besure AI Context

### 本地优先的上下文记忆系统 — 为 AI Agent 和人类设计

[English](README.md) | [中文](README_CN.md)

> 貔貅，只进不出，象征记忆一旦存入，永不丢失。

**Rust 引擎 · 本地部署 · 端到端加密 · MCP 原生 · 单二进制**

[English](README.md) | 中文

---

## 为什么需要 Besure AI Context？

你同时在做好几个项目。你不断在任务之间切换。每次切换，上下文就丢了——你在做什么、决定了什么、学到了什么。AI Agent 也有同样的问题。

**Besure AI Context 解决这个问题：**

| 痛点 | 方案 |
|------|------|
| 🔀 切换项目时**上下文丢失** | 类似 git branch 的上下文隔离——专注一个，秒级切换 |
| 🤖 **AI Agent 无法跨会话记忆** | 原生 MCP Server——Claude/Cursor/OpenClaw 可直接存取上下文 |
| ☁️ **云依赖和隐私担忧** | 100% 本地——SQLite + Markdown，零云服务 |
| 🔓 **数据安全** | AES-256-GCM + Argon2id 加密——密钥永不落盘 |
| 📦 **部署复杂** | 单二进制，零运行时依赖——`curl | bash` 即装即用 |

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

### 使用

```bash
# 初始化（启用加密）
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

# 导出分享
besure export "我的项目" -o project.md
```

---

## 接入 AI Agent（MCP）

Besure AI Context 内置 MCP（Model Context Protocol）Server，任何支持 MCP 的 AI 工具都能直接用：

### Claude Desktop / OpenClaw / Cursor

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

AI Agent 可以：
- **列出上下文** → 看到所有项目
- **添加记录** → 自动记录决策和进展
- **搜索记忆** → 找到相关的历史上下文
- **创建上下文** → 开始新项目记忆
- **导出分享** → 交接给同事

---

## Web Dashboard

```bash
besure serve --port 7788
# → 浏览器打开 http://localhost:7788
```

内置 Web 界面，浏览上下文、查看时间线、管理记录。主密码保护。

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
besure init --encrypt         初始化（启用加密）
besure create <title>         创建上下文
besure switch <query>         切换上下文（模糊匹配）
besure add <content>          添加记录（--type: milestone/decision/progress/blocker/note）
besure list                   列出所有上下文
besure log [context]          查看时间线
besure search <query>         全文搜索（--semantic 语义搜索）
besure absorb [--auto]        从对话提取记录（stdin 或 --from 文件）
besure export <context>       导出为 Markdown
besure serve [--port 7788]    启动 Web Dashboard + REST API
besure mcp                    启动 MCP Server (stdio)
besure unlock                 解锁
besure lock                   锁定
besure status                 查看状态
besure config set <k> <v>     配置 embedding/LLM API
```

---

## 路线图

| 阶段 | 状态 | 功能 |
|------|------|------|
| **MVP** | ✅ 完成 | 加密引擎、SQLite、CLI、Markdown 文件 |
| **V1** | ✅ 完成 | 向量检索、MCP Server、Absorb、REST API |
| **V2** | ✅ 完成 | Web Dashboard + 主密码认证 |
| **V3** | 🚧 进行中 | Tauri 桌面 APP、跨设备同步 |
| **V4** | 📋 计划中 | 团队协作、插件 SDK |
| **V5** | 📋 计划中 | VS Code 插件、浏览器插件 |

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
