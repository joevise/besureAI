#!/bin/bash
# Besure Dashboard Service Installer — 三平台进程守护
# 被 install.sh 调用，也可手动执行：bash scripts/install_service.sh
set -e

BIN_PATH="${BESURE_BIN:-$(which besure 2>/dev/null || echo "/usr/local/bin/besure")}"
BIN_DIR=$(dirname "$BIN_PATH")
OS_TYPE=$(uname -s)

echo "Installing Besure Dashboard service..."
echo "  Binary: $BIN_PATH"
echo "  Platform: $OS_TYPE"

case "$OS_TYPE" in
    Linux)
        SERVICE_DIR="$HOME/.config/systemd/user"
        mkdir -p "$SERVICE_DIR"

        cat > "$SERVICE_DIR/besure-dashboard.service" << EOF
[Unit]
Description=Besure AI Context Dashboard
After=network.target

[Service]
Type=simple
Environment=PATH=$BIN_DIR:/usr/local/bin:/usr/bin:/bin
ExecStart=$BIN_PATH serve --port 7788
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
EOF

        loginctl enable-linger "$USER" 2>/dev/null || true
        systemctl --user daemon-reload
        systemctl --user enable besure-dashboard.service
        systemctl --user restart besure-dashboard.service

        echo "✓ systemd service installed and started"
        echo "  Manage: systemctl --user {start|stop|status} besure-dashboard"
        ;;

    Darwin)
        PLIST_DIR="$HOME/Library/LaunchAgents"
        mkdir -p "$PLIST_DIR"
        PLIST_PATH="$PLIST_DIR/com.besure.context.plist"

        cat > "$PLIST_PATH" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.besure.context</string>
    <key>ProgramArguments</key>
    <array>
        <string>$BIN_PATH</string>
        <string>serve</string>
        <string>--port</string>
        <string>7788</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardErrorPath</key>
    <string>/tmp/besure-dashboard.err</string>
    <key>StandardOutPath</key>
    <string>/tmp/besure-dashboard.out</string>
</dict>
</plist>
EOF

        launchctl unload "$PLIST_PATH" 2>/dev/null || true
        launchctl load "$PLIST_PATH"

        echo "✓ launchd service installed and started"
        echo "  Manage: launchctl {load|unload} ~/Library/LaunchAgents/com.besure.context.plist"
        ;;

    MINGW*|MSYS*|CYGWIN*)
        STARTUP_DIR=$(cmd.exe /c "echo %APPDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup" 2>/dev/null | tr -d '\r')
        STARTUP_DIR=$(echo "$STARTUP_DIR" | sed 's|\\|/|g')

        WIN_BIN_PATH=$(echo "$BIN_PATH" | sed 's|/c/|C:/|; s|/d/|D:/|')

        cat > "$STARTUP_DIR/besure-dashboard.vbs" << EOF
Set WshShell = CreateObject("WScript.Shell")
WshShell.Run "$WIN_BIN_PATH serve --port 7788", 0, False
EOF

        echo "✓ Windows startup script installed"
        echo "  Location: $STARTUP_DIR/besure-dashboard.vbs"
        ;;

    *)
        echo "⚠️  Platform $OS_TYPE not recognized."
        echo "  You can manually run: besure serve --port 7788"
        ;;
esac

echo ""
echo "Dashboard: http://localhost:7788"
