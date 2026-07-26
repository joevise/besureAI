# Besure Skill — 多工具适配方案

## 设计
一个源文件 `skill/SKILL.md`（已完成），生成不同工具的格式：

| 工具 | 生成物 | 放置位置 |
|------|--------|---------|
| OpenClaw | SKILL.md + references/ (原样) | ~/.agents/skills/besure/ |
| Cursor | besure.mdc (frontmatter + MD) | ~/.cursor/rules/besure.mdc |
| Codex | AGENTS.md 片段 | ~/.codex/AGENTS.md (追加) |
| Windsurf | .windsurfrules | 项目/.windsurfrules |
| Claude Code | CLAUDE.md 片段 | ~/.claude/CLAUDE.md (追加) |
| Cline | .clinerules | 项目/.clinerules |
| Hermes | skill.md + metadata | ~/.hermes/skills/besure.md |
| Trae | .cursorrules | 项目/.cursorrules |
| WorkBuddy | 插件包 + settings.json | ~/.workbuddy/plugins/... |
| Aider | CONVENTIONS.md | 项目/CONVENTIONS.md |
| GitHub Copilot | copilot-instructions.md | 项目/.github/copilot-instructions.md |

## 各格式详细说明

### OpenClaw (已有)
直接复制 `skill/` 目录到 `~/.agents/skills/besure/`

### Cursor (.mdc 格式)
```markdown
---
description: Besure AI 闭环记忆系统
alwaysApply: true
---
（铁律 + 命令参考，纯 Markdown 内容）
```
放置：`~/.cursor/rules/besure.mdc`

### Codex (AGENTS.md)
纯 Markdown，追加到现有 AGENTS.md 或创建新的。
内容：铁律 + besure 命令列表。
放置：`~/.codex/AGENTS.md`

### Windsurf (.windsurfrules)
纯 Markdown rules。
放置：项目根目录 `.windsurfrules`

### Claude Code (CLAUDE.md)
纯 Markdown，追加到现有 CLAUDE.md 或创建。
放置：`~/.claude/CLAUDE.md`

### Cline (.clinerules)
纯 Markdown rules。
放置：项目根目录 `.clinerules`

### Hermes
Markdown skill，格式跟 OpenClaw 类似。
放置：`~/.hermes/skills/besure.md`

### Trae
跟 Cursor 兼容，`.cursorrules` 格式。
放置：项目根目录 `.cursorrules`

### WorkBuddy
插件包格式：需要创建目录 + settings.json 注册。
目录：`~/.workbuddy/plugins/marketplaces/codebuddy-plugins-official/plugins/besure/`
配置：`~/.workbuddy/settings.json` 加 `"besure@codebuddy-plugins-official": true`

### Aider
`CONVENTIONS.md` 格式。
放置：项目根目录

### GitHub Copilot
`.github/copilot-instructions.md`
放置：项目 `.github/` 目录

## 实现方式
Dashboard"获取 Skill"按钮 → 弹出工具选择列表 → 选工具 → 显示对应格式的内容 + 安装路径 → 复制/下载

不自动安装（让 Agent 自己装）。只生成正确格式的包。
