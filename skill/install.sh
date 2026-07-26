#!/bin/bash
# Besure Skill 一键安装
# 用法：bash install.sh

set -e

echo "🐹 Besure Skill 安装"
echo "===================="

# 1. 找 besure binary
BESURE_BIN=""
# 优先用 PATH 里的
if command -v besure &>/dev/null; then
  BESURE_BIN=$(which besure)
elif [ -f "$HOME/Library/Application Support/Besure/bin/besure" ]; then
  BESURE_BIN="$HOME/Library/Application Support/Besure/bin/besure"
fi

if [ -z "$BESURE_BIN" ]; then
  echo "❌ 未找到 besure CLI。"
  echo "   请先安装 Besure.app："
  echo "   curl -sSL https://raw.githubusercontent.com/joevise/besureAI/main/build-macos.sh | bash"
  exit 1
fi

echo "✅ 找到 besure: $BESURE_BIN"
$BESURE_BIN --version

# 2. 确保 binary 在 PATH
if [ "$BESURE_BIN" != "$(which besure 2>/dev/null)" ]; then
  mkdir -p "$HOME/.local/bin"
  ln -sf "$BESURE_BIN" "$HOME/.local/bin/besure"
  echo "✅ 软链到 ~/.local/bin/besure"
  if ! echo "$PATH" | grep -q ".local/bin"; then
    echo "⚠️  请把以下内容加到你的 ~/.zshrc 或 ~/.bashrc："
    echo '   export PATH="$HOME/.local/bin:$PATH"'
  fi
fi

# 3. 安装 skill
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SKILL_DIR="$HOME/.agents/skills/besure"

mkdir -p "$SKILL_DIR/references"

# 复制 skill 文件
cp "$SCRIPT_DIR/SKILL.md" "$SKILL_DIR/SKILL.md"
if [ -d "$SCRIPT_DIR/references" ]; then
  cp "$SCRIPT_DIR/references/"*.md "$SKILL_DIR/references/" 2>/dev/null || true
fi

echo "✅ Skill 安装到: $SKILL_DIR"
echo ""
echo "===================="
echo "🎉 安装完成！"
echo ""
echo "Skill 位置：$SKILL_DIR"
echo "Binary 位置：$BESURE_BIN"
echo ""
echo "现在你的 AI Agent（Codex/Cursor/Windsurf/OpenClaw）可以自动使用 Besure 记忆了。"
