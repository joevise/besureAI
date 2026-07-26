# Besure CLI 完整命令参考

## 初始化

```bash
besure init                    # 创建 vault（交互式设密码）
besure init --password ****    # 创建 vault（直接传密码）
besure unlock                  # 解锁 vault（交互式）
echo "password" | besure unlock  # 管道方式解锁
besure lock                    # 锁定 vault
besure status                  # 查看 vault 状态
```

## Context 管理

```bash
besure list                    # 列出所有 context
besure create "项目名"          # 创建新 context
besure create "项目名" --tag 标签
besure switch "关键词"          # 切换 context（模糊匹配）
besure delete context <id>     # 删除 context（入回收站）
besure restore <id>            # 从回收站恢复
besure trash                   # 查看回收站
besure purge <id>              # 永久删除（不可恢复）
```

## 添加记忆

```bash
# 基本添加
besure add "内容" --type milestone --context ctx_xxx

# 类型
besure add "决策内容" --type decision
besure add "踩坑记录" --type lesson
besure add "进展更新" --type progress
besure add "备注信息" --type note
besure add "待办问题" --type question
besure add "阻塞问题" --type blocker

# 生产级多段落记录
besure add --from-file entry.md --type decision --context ctx_xxx

# 自动标签（LLM 同步打标）
besure add "内容" --type milestone  # 自动打 1-3 个标签
```

## 搜索

```bash
# 全文搜索
besure search "关键词"

# 语义搜索（本地 AI，离线）
besure search "语义描述" --semantic

# 全局搜索（跨所有 context）
# search 默认搜当前 vault 的所有 context
```

## 查询

```bash
besure query                    # 当前 context 最近 20 条
besure query --all              # 跨所有 context
besure query --last 7d          # 最近 7 天
besure query --type decision    # 只要决策
besure query --keyword "V3"     # 关键词过滤
besure query --unresolved       # 只看未解决
besure log                      # 当前 context 时间线（紧凑格式）
```

## 记忆管理

```bash
besure resolve <id>             # 标记完成
besure append <id> "补充内容"   # 追加内容
besure link <id> --to <id> --as related_to   # 关联（caused_by/supersedes/related_to/ref_file/ref_commit/ref_url）
besure expire <id>              # 标记过期
besure supersede <old_id> <new_id>  # 标记替代
besure recall                   # 主动召回
besure stats                    # 统计
besure tags                     # 标签列表
besure retag --all              # 重新打标签
```

## Entry 操作

```bash
besure delete entry <id>        # 删除（入回收站）
besure restore <id>             # 恢复
```

## 导入导出

```bash
besure export --password ****              # 加密导出（.besure 格式）
besure import backup.besure --password **** # 加密导入
besure import backup.besure --context ctx_xxx  # 导入到指定 context
```

## 索引

```bash
besure index --all              # 全量建索引
besure index --context ctx_xxx  # 指定 context 建索引
besure index --all --rebuild    # 重建（删除旧的重新建）
```

## 配置

```bash
besure appconfig llm.provider openrouter
besure appconfig llm.api_url https://openrouter.ai/api/v1/chat/completions
besure appconfig llm.api_key sk-or-xxx
besure appconfig llm.model deepseek/deepseek-v4-flash
besure appconfig embedding.provider local
besure appconfig embedding.model bge-small-zh-v1.5
```

## Dashboard

```bash
besure serve --port 7788        # 启动 Dashboard Web 服务
```
