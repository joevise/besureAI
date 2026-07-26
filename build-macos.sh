#!/bin/bash
# Besure macOS 桌面 App — 一键编译脚本
# 在你的 Mac 上运行：bash build-macos.sh
# 前提：macOS 12+，已装 Xcode Command Line Tools

set -e

echo "🐹 Besure macOS App 编译脚本"
echo "=============================="
echo ""

# 1. 检查 Xcode CLI Tools
if ! xcode-select -p &>/dev/null; then
  echo "⚠️  需要先安装 Xcode Command Line Tools"
  echo "   运行：xcode-select --install"
  echo "   装完后重新跑这个脚本"
  exit 1
fi
echo "✅ Xcode Command Line Tools 已装"

# 2. 检查 Rust
if ! command -v rustc &>/dev/null; then
  echo "📦 安装 Rust..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi
echo "✅ Rust $(rustc --version)"

# 3. 检查 Tauri CLI
if ! command -v cargo-tauri &>/dev/null; then
  echo "📦 安装 Tauri CLI..."
  cargo install tauri-cli --version "^2.0" || true
fi
echo "✅ Tauri CLI 已装"

# 4. 克隆/更新代码
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if [ -d "$SCRIPT_DIR/src-tauri" ]; then
  # 已经在 repo 里
  cd "$SCRIPT_DIR"
  echo "✅ 在 repo 目录：$(pwd)"
else
  # 需要克隆
  echo "📦 克隆 Besure 仓库..."
  git clone https://github.com/joevise/besureAI.git ~/besureAI
  cd ~/besureAI
fi

echo ""
echo "📥 拉取最新代码..."
git pull origin main

echo ""
echo "🔨 编译 CLI binary（root crate）..."
cargo build --release

echo ""
echo "🔨 编译桌面 App..."
cd src-tauri
cargo tauri build

echo ""
echo "=============================="
echo "✅ 编译完成！"
echo ""
APP_PATH="target/release/bundle/macos/Besure AI Context.app"
DMG_PATH="target/release/bundle/dmg/"
echo "📱 App 位置：$APP_PATH"
echo "💿 DMG 位置：$DMG_PATH"
echo ""
echo "双击 .app 打开，或把 .dmg 拖进 Applications。"
echo ""
echo "⚠️  首次打开可能会提示'无法验证开发者'——"
echo "   右键 → 打开 → 确定（因为还没签名）"
