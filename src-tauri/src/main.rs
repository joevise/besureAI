// Besure AI Context — Desktop App Entry
use std::thread;
use tauri::Manager;

mod server;

const SERVER_PORT: u16 = 17_788;

fn main() {
    // 后台线程启动 HTTP 服务
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("failed to create runtime");
        rt.block_on(async {
            let _ = server::start_server(SERVER_PORT).await;
        });
    });

    std::thread::sleep(std::time::Duration::from_secs(1));

    tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval(&format!(
                    "window.location.href='http://localhost:{}'",
                    SERVER_PORT
                ));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
