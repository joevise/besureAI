# Besure AI Skill — 貔貅记忆

## Description
多上下文记忆管理。切换/记录/搜索/引用项目上下文。通过 CLI 调用本地 besure 二进制。

## When to Use
- 用户说"切换到XX项目" / "记一下XX" / "搜一下之前XX的记录"
- 需要跨任务/项目隔离上下文信息
- 需要记录进展、决策、阻碍
- 对话中需要搜索历史记录

## How to Use

所有操作通过 `exec` 调用 `besure` CLI 完成。

### 基础操作

```bash
# 列出所有上下文
besure list

# 创建新上下文
besure create "项目名" --tag 标签1 --tag 标签2 --summary "摘要"

# 切换上下文（支持模糊匹配）
besure switch "关键词"

# 添加进展记录
besure add "进展内容" --type progress      # 类型：init/milestone/decision/progress/blocker/note

# 查看时间线
besure log                                # 当前上下文
besure log "项目关键词"                    # 指定上下文

# 全文搜索
besure search "关键词"

# 语义搜索（需配置 embedding API）
besure search "意思相近的描述" --semantic

# 查看状态
besure status

# 从对话提取进展（自动）
echo "对话内容..." | besure absorb --auto

# 导出上下文
besure export "项目名" -o output.md

# 解锁/锁定
besure unlock    # 输入主密码
besure lock
```

### 使用流程

1. **Session 开始时**：`besure unlock` → `besure switch "当前项目"` → 加载上下文
2. **对话过程中**：`besure add "重要决策或进展"` 随时记录
3. **Session 结束前**：`besure lock` 锁定

### 注意事项

- 每次调用 besure 都是独立进程，unlock 后状态持久（明文 .db 存在直到 lock）
- 加密模式下必须先 unlock 才能操作
- search 默认全文匹配；--semantic 需配置 embedding API
- absorb --auto 会自动将提取的记录添加到当前上下文
