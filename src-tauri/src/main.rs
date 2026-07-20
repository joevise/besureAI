// Besure AI Context — Desktop App Entry
use std::thread;

mod server;

const SERVER_PORT: u16 = 17_788;

fn main() {
    // 后台线程启动 HTTP 服务
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        rt.block_on(async {
            if let Err(e) = server::start_server(SERVER_PORT).await {
                eprintln!("HTTP server error: {}", e);
            }
        });
    });

    // 等待 HTTP 服务就绪
    std::thread::sleep(std::time::Duration::from_millis(500));

    // GPU/渲染回退：优先软件渲染，避免 DRM 权限问题
    if std::env::var("LIBGL_ALWAYS_SOFTWARE").is_err() {
        std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
    }
    if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
