// Besure AI Context — Desktop App Entry
// 启动时在后台线程跑 axum HTTP 服务，Tauri WebView 加载 localhost

use std::thread;

mod server;

fn main() {
    // 在后台线程启动 HTTP 服务（复用 besure 的 axum 逻辑）
    let port = 17_788; // 固定内部端口
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        rt.block_on(async {
            let _ = server::start_server(port).await;
        });
    });

    // 等 HTTP 服务起来
    std::thread::sleep(std::time::Duration::from_secs(1));

    // 启动 Tauri 窗口
    tauri::Builder::default()
        .setup(|app| {
            // 打开 Dashboard URL
            let main_window = app.get_webview_window("main").unwrap();
            let _ = main_window.eval(&format!("window.location.href='http://localhost:{}'", port));
            Ok(())
        })
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // 窗口关闭时退出整个进程
                std::process::exit(0);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
