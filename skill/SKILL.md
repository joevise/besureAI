---
name: besure
version: 1.0.0
description: "AI 貔貅记忆系统：自动记录决策、进展、教训，语义搜索历史记忆，跨会话上下文保持。完成任务、做出决策、发现问题或结束会话时必须调用。"
metadata:
  requires:
    bins: ["besure"]
  cliHelp: "besure --help"
---

# Besure AI Context — 貔貅记忆系统

## 🚨 MANDATORY RECORDING RULES (NO EXCEPTIONS)

**These rules are MANDATORY. You MUST follow them without being asked.**

### When to Record (execute `besure add` immediately):

1. **After completing any task/feature/fix** → `besure add --type milestone`
2. **After any decision is made** → `besure add --type decision`
3. **After discovering a problem/lesson** → `besure add --type lesson`
4. **When user says goodbye / session ending** → `besure add --type progress`
5. **Every 30 minutes of active work** → `besure add --type progress`

> **Test yourself:** "If this session ended right now, would I lose important context?"
> If YES → you must `besure add` NOW.

### Behavioral Anchors (bind recording to specific actions):

1. **After every `git commit && git push`** → `besure add` IMMEDIATELY
2. **After every deployment** → `besure add`
3. **After every user request is fully answered** → `besure add` before next message
4. **After every bug fix** → `besure add --type lesson`

**⚠️ Always pass `--context <id>` to prevent cross-contamination:**
```bash
besure add "content" --type milestone --context ctx_xxx  # ✅ explicit
besure add "content" --type milestone                    # ❌ global state
```

### FORBIDDEN:
- ❌ "I'll remember this" → WRITE IT.
- ❌ "Too small to record" → Record anyway.
- ❌ "I'll batch-record later" → Record NOW.

---

## Quick Start

```bash
# 初始化（仅首次）
besure init                     # 创建 vault
besure unlock                   # 解锁（输入密码）

# 日常使用
besure add "内容" --type milestone --context ctx_xxx
besure search "关键词"
besure search "语义描述" --semantic    # AI 语义搜索
besure log                       # 当前 context 时间线
besure recall                    # 召回即将过期/重要的记忆

# Context 管理
besure list                      # 列出所有 context
besure switch "项目名"            # 切换 context
besure create "新项目"            # 创建新 context

# 索引（语义搜索前需要建索引）
besure index --all               # 给存量数据建向量索引
```

## 命令参考

| 命令 | 说明 |
|------|------|
| `besure add "text" --type T --tags A,B --context C` | 添加记忆（type: milestone/decision/lesson/progress/note/question/blocker） |
| `besure search "query"` | 全文搜索 |
| `besure search "query" --semantic` | 语义搜索（本地 AI，离线） |
| `besure list` | 列出所有 context |
| `besure switch "keyword"` | 切换 context（模糊匹配） |
| `besure create "name"` | 创建新 context |
| `besure log` | 当前 context 时间线 |
| `besure query --last 7d` | 查询最近 7 天 |
| `besure query --type decision` | 只看决策 |
| `besure query --all` | 跨所有 context 查询 |
| `besure recall` | 主动召回（即将过期/最近/被替代） |
| `besure stats` | 统计概览 |
| `besure tags` | 标签列表 |
| `besure resolve <id>` | 标记完成 |
| `besure append <id> "补充"` | 追加内容 |
| `besure profile` | 查看当前项目 Profile（git/服务器/密码等） |
| `besure profile set <key> <value>` | 设置项目信息 |
| `besure profile delete <key>` | 删除项目信息 |
| `besure link <id> --to <id> --as related_to` | 关联记忆 |
| `besure delete entry <id>` | 删除（入回收站） |
| `besure restore <id>` | 从回收站恢复 |
| `besure export --password ***` | 加密导出 |
| `besure import backup.besure --password ***` | 加密导入 |
| `besure index --all` | 建立向量索引 |
| `besure --version` | 版本号 |

## Tag 规则（重要）

### 创建新 Context 时：
1. `besure create "项目名"` 创建 context
2. 立即 `besure profile set git_repo <url>` 填写项目基础信息
3. `besure switch "项目名"` 切换到新 context

调用 `besure add` 时，**必须自己生成 tags 传进来**（你本身就是大模型，不需要额外调 API）：

1. **先看已有标签**：运行 `besure tags` 获取当前标签库
2. **优先复用**：从已有标签里选合适的，**不要造同义词**
3. **不够才新建**：已有标签确实没有合适的，才创建新标签
4. **大类原则**：标签必须是宽泛大类（如：后端开发、部署、家庭、投资、产品规划），不要具体名词
5. **最多3个**：每次最多传 3 个标签

```bash
besure tags                                                        # 先看已有的
besure add "内容" --type milestone --tags 部署,安装 --context ctx_xxx  # 自己生成 tag
```

## Semantic Search

- 默认全文匹配；`--semantic` 走本地 fastembed（bge-small-zh-v1.5，512维，完全离线）
- 首次使用自动下载模型（~100MB）
- 中文长文本效果最好；英文/专有名词用关键词搜索

完整命令参考：[`references/commands.md`](references/commands.md)
