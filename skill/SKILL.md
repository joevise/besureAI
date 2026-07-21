# Besure AI Skill — 闭环记忆系统

## 🚨 MANDATORY RECORDING RULES V0.3 (NO EXCEPTIONS)

**These rules are MANDATORY. You MUST follow them without being asked.**

> **Iron Rules V0.3** — Added Anti-cross-contamination rule: always use `--context <id>` with `besure add` to prevent data corruption.
> V0.2: Behavioral Anchors (git push → add, deploy → add)
> V0.1: Basic rules (5 mandatory recording scenarios)

### When to Record (execute `besure add` immediately):

1. **After completing any task/feature/fix** → `besure add --type milestone`
2. **After any decision is made** → `besure add --type decision`
3. **After discovering a problem/lesson** → `besure add --type lesson`
4. **When user says goodbye / session ending** → `besure add --type progress` (summary)
5. **Every 30 minutes of active work** → `besure add --type progress`

### Test yourself:
> "If this session ended right now, would I lose important context?"
> If YES → you must `besure add` NOW.
> When in doubt, RECORD. Over-recording is cheap. Lost memory is expensive.

### Behavioral Anchors (CRITICAL — bind recording to specific actions):

Tie `besure add` to these concrete trigger points — NOT to abstract "task completion":

1. **After every `git commit && git push`** → `besure add` IMMEDIATELY (before doing anything else)
2. **After every deployment** (copy binary, restart service, curl test) → `besure add`
3. **After every user request is fully answered** → `besure add` before waiting for next message
4. **After every bug fix** → `besure add --type lesson` (what was the bug, root cause, fix)

**⚠️ Anti-cross-contamination (CRITICAL):** Always pass `--context <id>` to `besure add`:
```bash
besure add "content" --type milestone --context ctx_xxx  # ✅ explicit
besure add "content" --type milestone                    # ❌ uses global state
```
Never rely on "current context" — it's global mutable state that causes data corruption when you forget to switch back.

The danger zone is the "flow state" — when you're coding/debugging, you forget to record.
The fix: record AS PART OF the commit/deploy workflow, not as a separate "I'll do it later" step.

### FORBIDDEN:
- ❌ "I'll remember this for later" → You won't. WRITE IT.
- ❌ "This is too small to record" → Record it anyway.
- ❌ "I'll batch-record at the end" → Record NOW.
- ❌ "I'm in the middle of something, I'll record after the next step" → Record NOW, then continue.

---

## Description
多上下文记忆管理。切换/记录/搜索/关联/召回。基于闭环逻辑设计：8 维度（主体/编码/完整性/上下文/检索/关联/时效/失效）。通过 CLI 调用本地 besure 二进制。

## When to Use
- 用户说"切换到XX项目" / "记一下XX" / "搜一下之前XX的记录"
- 需要跨任务/项目隔离上下文信息
- 需要记录进展、决策、阻碍、配置、教训
- 对话中需要搜索历史记录
- 需要关联记忆（因果/替代/引用）
- 需要标记过期/被替代的决策
- 需要主动召回近期需要注意的记忆
- Session 结束前需要保存上下文
- **完成任何任务后必须记录**（强制）
- **做了决策后必须记录**（强制）

## How to Use

所有操作通过 `exec` 调用 `besure` CLI 完成。每次调用前确保 PATH 包含 besure：
```bash
export PATH="$HOME/.hermes/node/bin:$PATH"
```

### 基础操作

```bash
# 列出所有上下文
besure list

# 创建新上下文
besure create "项目名" --tag 标签1 --summary "摘要"

# 切换上下文（支持模糊匹配）
besure switch "关键词"

# 查看状态
besure status

# 解锁/锁定
besure unlock    # 输入主密码
besure lock
```

### 记录管理

```bash
# 添加进展记录（快速模式）
besure add "一句话内容" --type progress
# 类型：init/milestone/decision/progress/blocker/note/config/lesson/question

# 添加多段落 Markdown 记录（生产级颗粒度）
besure add --from-file entry.md --type decision

# 查看时间线
besure log
besure log "项目关键词"

# 全文搜索
besure search "关键词"

# 语义搜索（需配置 embedding API）
besure search "意思相近的描述" --semantic

# 导出上下文
besure export "项目名" -o output.md
```

### V3 闭环功能

```bash
# === F 关联 ===
# 给 entry 建立关联
besure link <entry_id> --to <target_id> --as <relation>
# relation: caused_by / supersedes / related_to / ref_file / ref_commit / ref_url

# === H 失效 ===
# 标记 entry 过期
besure expire <entry_id>

# 标记旧 entry 被新 entry 替代
besure supersede <old_entry_id> <new_entry_id>

# === G 时效 / E 主动召回 ===
# 查看需要注意的记忆（即将过期/已过期/最近/被替代）
besure recall
```

### V0.4 查询 & 管理功能

```bash
# === 统一查询（Agent 友好的紧凑输出）===
# 默认：当前上下文最近 20 条
besure query
# 时间过滤
besure query --last 7d              # 最近 7 天
besure query --from 2026-07-01 --to 2026-07-18
# 类型过滤（可重复 --type）
besure query --type decision --type milestone
# 跨上下文
besure query --all
besure query --context "besure"
# 关键词 + resolved 过滤
besure query --keyword "V3" --unresolved
# 组合
besure query --all --last 7d --type milestone

# === 标记完成 ===
besure resolve <entry_id>

# === 追加内容 ===
besure append <entry_id> "补充内容"
besure append <entry_id> --from-file supplement.md

# === 统计概览 ===
besure stats
```

### V0.5 多 Vault 架构

每个 Agent 默认有自己的 vault（物理隔离），通过环境变量配置：
```bash
# 环境变量
BESURE_VAULT=~/.besure/joey/          # 当前 Agent 的 vault
BESURE_VAULTS_ALL=true                 # 全局视角（只给主 Agent）
BESURE_VAULT_ROOT=~/.besure/           # vault 父目录（扫描用）
BESURE_SHARED_VAULT=~/.besure/shared/   # 共享 vault 路径
```

```bash
# 列出所有 vault（需全局视角）
besure vaults

# 跨 vault 查询
besure query --all-vaults

# 推送到共享空间
besure share <entry_id>
besure share-context <context_id>

# 查看共享内容
besure shared
besure shared --keyword "BTC"
```

### V0.58 自动标签（Emergent Tagging）

`besure add` 时自动调用 LLM 给 entry 打 1-3 个扁平大类中文标签（如 `后端开发`、`部署`、`家庭`、`投资`），同步写入 `entry.tags`：

```bash
# add 时自动打标（输出带 🏷 标签）
besure add "完成了后端 API 部署" --context ctx_xxx
# → ✓ Added progress entry to ctx_xxx  🏷 后端开发, 部署

# 查看标签库（tag + 使用次数，降序）
besure tags

# 给存量 entry 补标签
besure retag                    # 当前 context
besure retag --context ctx_xxx  # 指定 context
besure retag --all              # 全库
```

要点：
- 标签库存 `tag_vocab` 表：新标签先匹配已有库，语义相同复用（避免"前端"/"frontend"同义词爆炸），没有才新建
- LLM 不可用（provider=dummy / 无 api_url / 请求失败）时**降级跳过**，绝不阻塞 add
- 打标复用 `~/.besure/appconfig.json` 的 `llm` 配置（同 absorb）
- Dashboard 每条 entry 显示标签，可按标签筛选（前端过滤）
- MCP tool `besure_list_tags` 返回标签库；REST `GET /api/tags`

### 项目配置管理

```bash
# 存储项目配置（仓库地址、服务器、密钥引用等）
besure config set repo "https://github.com/user/project"
besure config set server "67.209.190.54:7788"
besure config set deploy_cmd "ssh root@server && systemctl restart app"

# 读取配置
besure config get repo

# 列出所有配置
besure config list
```

### 生产级记录格式（推荐）

对重要决策和里程碑，使用 `--from-file` 写多段落 Markdown：

```markdown
## 决策/事件标题

### 做了什么
具体行动描述。

### 为什么
决策理由、权衡逻辑。

### 踩坑
遇到的问题和解法。

### 关联
- commit: abc123
- 文件: src/xxx.rs
- 决策人: 大Joe
```

### 使用流程

1. **Session 开始时**：
   - `besure unlock`（如加密）
   - `besure switch "当前项目"`
   - `besure recall`（查看需要注意的记忆）

2. **对话过程中**：
   - `besure add "快速记录"` 或 `besure add --from-file entry.md`
   - `besure config set key value`（项目配置）
   - `besure link <id> --to <id>`（建立关联）

3. **Session 结束前**：
   - 重要决策/进展 → `besure add --from-file`
   - `besure lock`（如加密）

### 从对话提取进展

```bash
echo "对话内容..." | besure absorb --auto
```

### 注意事项

- 每次调用 besure 都是独立进程
- 加密模式下必须先 unlock 才能操作
- search 默认全文匹配；--semantic 需配置 embedding API
- V3 新字段（links/status/valid_until/superseded_by/resolved）向下兼容，旧 entry 不受影响
- V0.4 query 默认返回 20 条，紧凑格式（对 Agent 友好）
- V0.4 resolve 标记完成，append 追加内容（加分隔线+时间戳）
- DB migration 幂等，多次运行安全
- MCP Server 24 个 tools（含 query/resolve/append/stats/list_tags）
- Dashboard 支持 filter bar、resolved 徽章、append 输入框、Stats Tab、Tags Tab、标签筛选
- V0.58 自动标签：add 时 LLM 同步打标（1-3 个大类），tag_vocab 复用防同义词；LLM 不可用自动降级
