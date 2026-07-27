// Besure AI Context — Desktop App Entry
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use tauri::Manager;

mod server;
mod tray;

/// App 数据目录：macOS 上为 ~/Library/Application Support/Besure/
/// 用于存 config.json、Dashboard 配置等
pub fn app_data_dir() -> PathBuf {
    let base = dirs::data_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Besure")
}

/// Vault 统一目录：~/.besure/
/// App、CLI、所有 Agent 都用这同一个目录
pub fn vault_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".besure")
}

/// 从 start 开始找一个空闲端口
fn find_free_port(start: u16) -> u16 {
    (start..start + 1000)
        .find(|p| TcpListener::bind(("127.0.0.1", *p)).is_ok())
        .expect("no free port available")
}

/// 轮询等服务端口可连接
fn wait_for_server(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    eprintln!("⚠️  server on port {} not ready after 30s", port);
}

fn main() {
    // 1. 统一 vault 目录：~/.besure/（跟 CLI 完全一致）
    // 不设 BESURE_VAULT_ROOT — 让 vault_parent() 自然 fallback 到 ~/，
    // 这样扫描 ~/ 下的子目录就能找到 ~/.besure/ 作为一个 vault
    let data_dir = app_data_dir();
    std::fs::create_dir_all(&data_dir).ok();
    let vaults = vault_root();
    std::fs::create_dir_all(&vaults).ok();
    std::env::set_var("BESURE_VAULTS_ALL", "true");

    // 恢复 Dashboard 密码（onboarding 时写入 config.json）
    if let Ok(json) = std::fs::read_to_string(data_dir.join("config.json")) {
        if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&json) {
            if let Some(pw) = cfg.get("dashboard_password").and_then(|v| v.as_str()) {
                std::env::set_var("BESURE_DASHBOARD_PASSWORD", pw);
            }
        }
    }

    // 2. 动态端口：对外端口 + 内部 ApiServer 端口
    let port = find_free_port(17_000);
    let internal_port = find_free_port(port + 1);

    // 3. 后台线程启动内嵌服务
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        rt.block_on(async {
            if let Err(e) = server::start_server(port, internal_port).await {
                eprintln!("HTTP server error: {}", e);
            }
        });
    });

    // 4. 等服务就绪
    wait_for_server(port);

    // GPU/渲染回退（Linux 开发环境防 DRM 权限问题）
    if std::env::var("LIBGL_ALWAYS_SOFTWARE").is_err() {
        std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
    }
    if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    // 5. Tauri 启动，窗口在 setup 回调中用动态 URL 创建
    let url = format!("http://localhost:{}", port);
    tauri::Builder::default()
        .setup(move |app| {
            tray::setup_tray(app)?;

            // 部署 CLI binary：bundle Resources/besure → ~/Library/Application Support/Besure/bin/besure
            if let Ok(resource) = app
                .path()
                .resolve("besure", tauri::path::BaseDirectory::Resource)
            {
                if resource.exists() {
                    let target = vaults.join("bin").join("besure");
                    let deployed = (|| -> std::io::Result<()> {
                        if let Some(parent) = target.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::copy(&resource, &target)?;
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            std::fs::set_permissions(
                                &target,
                                std::fs::Permissions::from_mode(0o755),
                            )?;
                        }
                        Ok(())
                    })();
                    match deployed {
                        Ok(_) => println!("✅ CLI binary deployed to {}", target.display()),
                        Err(e) => eprintln!("⚠️  failed to deploy CLI binary: {}", e),
                    }
                } else {
                    eprintln!(
                        "⚠️  CLI binary not found in bundle resources: {}",
                        resource.display()
                    );
                }
            }

            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External(tauri::Url::parse(&url).expect("invalid server url")),
            )
            .title("Besure")
            .inner_size(1200.0, 800.0)
            .min_inner_size(900.0, 600.0)
            .build()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关窗口 → 隐藏到托盘，不退出
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
