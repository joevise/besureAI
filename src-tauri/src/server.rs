// Besure AI Context — Desktop App Embedded Server
// 完全复用 besure_lib 的 ApiServer（跑在内部端口），
// 对外端口叠加 onboarding 端点 + 反向代理，不重写 REST API

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;

use besure_lib::ai::rest_api::ApiServer;
use besure_lib::storage::Vault;

#[derive(Clone)]
struct ProxyState {
    internal_port: u16,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct OnboardingStatus {
    needed: bool,
}

#[derive(Deserialize)]
struct SetupBody {
    password: String,
}

/// 默认 vault 的配置文件路径：~/.besure/.besure.config（跟 CLI besure init 一致）
fn default_vault_config_path() -> PathBuf {
    Vault::default_root().join(".besure.config")
}

fn err_response(code: StatusCode, msg: &str) -> Response {
    (
        code,
        Json(serde_json::json!({"ok": false, "error": msg})),
    )
        .into_response()
}

pub async fn start_server(public_port: u16, internal_port: u16) -> anyhow::Result<()> {
    // 内部端口跑原版 ApiServer（完全复用 besure_lib，环境变量已在 main.rs 设好）
    let internal = tokio::spawn(async move {
        if let Err(e) = ApiServer::new(internal_port).run().await {
            eprintln!("internal ApiServer error: {}", e);
        }
    });

    let state = Arc::new(ProxyState {
        internal_port,
        client: reqwest::Client::new(),
    });

    let app = Router::new()
        .route("/api/onboarding", get(onboarding_status))
        .route("/api/onboarding/setup", post(onboarding_setup))
        .fallback(proxy_to_internal)
        .with_state(state);

    // 桌面 App 只监听本机回环
    let addr = format!("127.0.0.1:{}", public_port);
    let listener = TcpListener::bind(&addr).await?;
    println!(" Besure Desktop on http://localhost:{}", public_port);
    axum::serve(listener, app).await?;
    internal.abort();
    Ok(())
}

/// GET /api/onboarding → {needed: bool}
async fn onboarding_status() -> Json<OnboardingStatus> {
    Json(OnboardingStatus {
        needed: !default_vault_config_path().exists(),
    })
}

/// POST /api/onboarding/setup {password} → 创建默认 vault 并设置 Dashboard 密码
async fn onboarding_setup(Json(body): Json<SetupBody>) -> Response {
    if body.password.len() < 4 {
        return err_response(StatusCode::BAD_REQUEST, "password too short (min 4 chars)");
    }
    let config_path = default_vault_config_path();
    if config_path.exists() {
        return err_response(StatusCode::CONFLICT, "vault already initialized");
    }
    let vault_dir = config_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    if let Err(e) = Vault::init(Some(vault_dir), Some(&body.password)) {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("vault init failed: {}", e),
        );
    }
    std::env::set_var("BESURE_DASHBOARD_PASSWORD", &body.password);

    // 持久化到 App 级 config.json，重启后由 main.rs 恢复
    let data_dir = crate::app_data_dir();
    let app_config_path = data_dir.join("config.json");
    let mut cfg: serde_json::Value = std::fs::read_to_string(&app_config_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    cfg["dashboard_password"] = serde_json::Value::String(body.password);
    if let Err(e) = std::fs::write(&app_config_path, serde_json::to_string_pretty(&cfg).unwrap()) {
        eprintln!("⚠️  failed to persist app config: {}", e);
    }

    Json(serde_json::json!({"ok": true})).into_response()
}

/// 反向代理：其余请求转发给内部 ApiServer，保持同源（无 CORS 问题）
async fn proxy_to_internal(State(state): State<Arc<ProxyState>>, req: Request) -> Response {
    let (parts, body) = req.into_parts();
    let path_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let url = format!("http://127.0.0.1:{}{}", state.internal_port, path_query);

    let body_bytes = match to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => return err_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };

    let mut rb = state.client.request(parts.method, &url);
    for (k, v) in &parts.headers {
        if k == "host" {
            continue;
        }
        rb = rb.header(k, v);
    }

    let resp = match rb.body(body_bytes).send().await {
        Ok(r) => r,
        Err(e) => return err_response(StatusCode::BAD_GATEWAY, &e.to_string()),
    };

    let mut builder = Response::builder().status(resp.status());
    for (k, v) in resp.headers() {
        builder = builder.header(k, v);
    }
    let bytes = resp.bytes().await.unwrap_or_default();
    builder
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
