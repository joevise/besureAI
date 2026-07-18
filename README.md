# 貔貅记忆 Besure AI

### Context Switch Memory System — 通用多上下文管理系统

> 貔貅，只进不出，象征记忆一旦存入，永不丢失。

**Rust 引擎 · 本地部署 · 端到端加密 · CLI + MCP + REST API**

---

## 安装

### 方式一：一键安装（推荐）

```bash
curl -fsSL https://raw.githubusercontent.com/joevise/besureAI/main/install.sh | bash
```

### 方式二：从 Release 下载预编译二进制

前往 [Releases](https://github.com/joevise/besureAI/releases) 下载对应平台的二进制文件。

### 方式三：从源码编译

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # 安装 Rust（如果没有）
git clone https://github.com/joevise/besureAI.git
cd besureAI
cargo install --path .
```

### 方式四：cargo install

```bash
cargo install besure
```

---

## 快速开始

```bash
# 初始化（设置主密码，开启加密）
besure init --encrypt

# 解锁
besure unlock

# 创建第一个上下文
besure create "我的项目" --tag rust --summary "项目摘要"

# 记录进展
besure add "完成了第一版设计" --type milestone
besure add "决定用 Rust" --type decision
besure add "遇到编译错误，已修" --type blocker

# 查看所有上下文
besure list

# 查看时间线
besure log

# 搜索
besure search "关键词"
besure search "意思相近的内容" --semantic   # 语义搜索（需配置 embedding API）

# 切换上下文
besure switch "项目名"   # 支持模糊匹配

# 导出
besure export "项目名" -o project.md

# 从对话自动提取进展
echo "今天完成了MCP Server\n决定用axum做API" | besure absorb --auto

# 锁定
besure lock
```

---

## 核心能力

| 能力 | 说明 |
|------|------|
| 🔒 端到端加密 | AES-256-GCM + Argon2id 密钥派生，密钥永不落盘 |
| 🗂️ 多上下文管理 | 创建/切换/搜索，像 git branch 一样管理任务上下文 |
| 🔍 语义搜索 | 内置向量检索，自然语言秒搜历史记录 |
| 🤖 MCP Server | stdio JSON-RPC，Claude/OpenClaw/Cursor 可直接接入 |
| 🌐 REST API | `besure serve` 启动 HTTP 服务，第三方可调用 |
| 📝 Markdown | 人可读的 Markdown + JSON frontmatter 文件 |
| 💾 本地优先 | SQLite + 文件，数据在你手里，离线可用 |

---

## CLI 命令

```
besure init --encrypt       初始化 vault
besure create <title>       创建上下文
besure switch <query>       切换上下文（模糊匹配）
besure add <content>        添加进展
besure list                 列出所有上下文
besure log                  查看时间线
besure search <query>       搜索（全文/语义）
besure export <context>     导出为 Markdown
besure absorb               从对话提取进展
besure unlock               解锁
besure lock                 锁定
besure status               查看状态
besure serve --port 7788    启动 REST API
besure mcp                  启动 MCP Server
besure config set <k> <v>   配置管理
```

## 配置 Embedding API

```bash
# 配置 OpenAI embedding（用于语义搜索）
besure config set embedding.provider openai
besure config set embedding.api_url https://api.openai.com/v1/embeddings
besure config set embedding.api_key sk-xxx
besure config set embedding.model text-embedding-3-small

# 或 MiniMax
besure config set embedding.provider minimax
besure config set embedding.api_url https://api.minimaxi.com/v1/embeddings
besure config set embedding.api_key sk-xxx
```

## MCP Server 接入

在 Claude Desktop / OpenClaw / Cursor 配置中添加：

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

## 安全

- AES-256-GCM 军用级加密
- Argon2id 密钥派生（64MB 内存 / 3 次迭代 / 4 线程）
- 密钥只存在内存，lock 时 zeroize 清除
- 每个文件独立加密，单文件泄露不影响其他
- 无任何云依赖，数据完全本地

---

## 技术栈

纯 Rust，单二进制，零外部依赖：

| 组件 | Crate |
|------|-------|
| 加密 | aes-gcm + argon2 + zeroize |
| 数据库 | rusqlite (bundled) |
| CLI | clap |
| HTTP | reqwest (rustls-tls) |
| REST API | axum + tokio |
| 序列化 | serde + serde_json |

## License

MIT
