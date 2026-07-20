#!/usr/bin/env bash
# 貔貅记忆 Besure AI — 一键安装脚本
# 用法: curl -fsSL https://raw.githubusercontent.com/joevise/besureAI/main/install.sh | bash

set -euo pipefail

REPO="joevise/besureAI"
INSTALL_DIR="${1:-/usr/local/bin}"
BINARY_NAME="besure"

# 检测平台
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin) OS="darwin" ;;
    Linux)  OS="linux" ;;
    *) echo "❌ 不支持的系统: $OS"; exit 1 ;;
esac

case "$ARCH" in
    arm64|aarch64) ARCH="arm64" ;;
    x86_64|amd64)  ARCH="x64" ;;
    *) echo "❌ 不支持的架构: $ARCH"; exit 1 ;;
esac

ARTIFACT="${BINARY_NAME}-${OS}-${ARCH}"
URL="https://github.com/${REPO}/releases/latest/download/${ARTIFACT}"

echo "🐉 貔貅记忆 Besure AI — 安装中..."
echo "   平台: ${OS}-${ARCH}"
echo "   下载: ${URL}"
echo ""

# 下载
TEMP_FILE=$(mktemp)
if command -v curl &> /dev/null; then
    curl -fsSL "${URL}" -o "${TEMP_FILE}"
elif command -v wget &> /dev/null; then
    wget -qO "${TEMP_FILE}" "${URL}"
else
    echo "❌ 需要 curl 或 wget"
    exit 1
fi

chmod +x "${TEMP_FILE}"

# 安装
if [ -w "${INSTALL_DIR}" ]; then
    mv "${TEMP_FILE}" "${INSTALL_DIR}/${BINARY_NAME}"
else
    echo "🔐 需要 sudo 权限安装到 ${INSTALL_DIR}"
    sudo mv "${TEMP_FILE}" "${INSTALL_DIR}/${BINARY_NAME}"
fi

echo ""
echo "✅ 安装成功！${BINARY_NAME} → ${INSTALL_DIR}/${BINARY_NAME}"
echo ""

# 安装进程守护（auto-start + auto-restart）
echo "🔧 安装 Dashboard 进程守护..."
if "${INSTALL_DIR}/${BINARY_NAME}" service install 2>/dev/null; then
    echo "✓ Dashboard 服务已启动，开机自启已配置"
else
    echo "⚠️  进程守护安装失败（不影响正常使用）"
    echo "   可手动启动: besure serve --port 7788"
fi

echo ""
echo "开始使用："
echo "  besure setup --agent-name \"Joey\"   # 一键配置（推荐）"
echo "  # ↑ 自动初始化 + 检测 Agent 配置 + 注入强制记忆规则"
echo ""
echo "  或手动："
echo "  besure init --encrypt          # 初始化（设置主密码）"
echo "  besure create \"我的项目\"       # 创建上下文"
echo "  besure add \"完成了某件事\"      # 记录进展"
echo "  besure list                    # 查看所有上下文"
echo "  besure service status          # 查看 Dashboard 状态"
echo "  besure --help                  # 查看所有命令"
echo ""
echo "貔貅记忆 — 只进不出，记忆永存 🐉"
