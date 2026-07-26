#!/bin/bash
# Besure macOS 桌面 App — 一键编译脚本（支持重复运行）
# 首次：clone + 编译；后续：更新 + 编译

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
fi
source "$HOME/.cargo/env" 2>/dev/null || true
echo "✅ Rust $(rustc --version)"

# 3. 检查 Tauri CLI
if ! command -v cargo-tauri &>/dev/null; then
  echo "📦 安装 Tauri CLI..."
  cargo install tauri-cli --version "^2.0" || true
fi
echo "✅ Tauri CLI 已装"

# 4. Clone 或更新代码
REPO_DIR="$HOME/besureAI"
if [ -d "$REPO_DIR/.git" ]; then
  echo "📥 更新已有仓库..."
  cd "$REPO_DIR"
  git stash -q 2>/dev/null || true
  git pull origin main
else
  echo "📦 克隆 Besure 仓库..."
  git clone https://github.com/joevise/besureAI.git "$REPO_DIR"
  cd "$REPO_DIR"
fi

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
# 找到产物路径
APP_PATH=$(find target/release/bundle/macos -name "*.app" 2>/dev/null | head -1)
DMG_PATH=$(find target/release/bundle/dmg -name "*.dmg" 2>/dev/null | head -1)

# 把 CLI binary 打进 App bundle Resources（App 启动时会部署到数据目录）
if [ -n "$APP_PATH" ]; then
  if [ -f ../target/release/besure ]; then
    cp ../target/release/besure "$APP_PATH/Contents/Resources/besure"
    chmod +x "$APP_PATH/Contents/Resources/besure"
    echo "✅ CLI binary 已打包：$APP_PATH/Contents/Resources/besure"
  else
    echo "⚠️  未找到 ../target/release/besure，App 将无法部署 CLI"
  fi
fi

if [ -n "$APP_PATH" ]; then
  echo "📱 App：$APP_PATH"
fi
if [ -n "$DMG_PATH" ]; then
  echo "💿 DMG：$DMG_PATH"
fi
echo ""
echo "⚠️  首次打开会提示'无法验证开发者'——"
echo "   右键 → 打开 → 确定"
echo ""
echo "🧹 如果要重新体验首次设置："
echo "   rm -rf ~/Library/Application\\ Support/Besure/"
