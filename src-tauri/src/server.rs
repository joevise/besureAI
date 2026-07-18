// 内嵌 HTTP 服务器 — 复用 besure core 的逻辑
// 桌面 APP 自带 HTTP 服务，Tauri WebView 加载它

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
    vault_root: std::path::PathBuf,
    sessions: Arc<Mutex<HashSet<String>>>,
}

#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub async fn start_server(port: u16) -> anyhow::Result<()> {
    let state = AppState {
        vault_root: besure::storage::Vault::default_root(),
        sessions: Arc::new(Mutex::new(HashSet::new())),
    };

    let app = Router::new()
        .route("/", get(dashboard))
        .route("/api/health", get(health))
        .route("/api/auth", post(auth))
        .route("/api/auth/check", get(auth_check))
        .route("/api/auth/logout", post(logout))
        .route("/api/contexts", get(list_contexts))
        .route("/api/contexts", post(create_context))
        .route("/api/contexts/:id/log", get(get_log))
        .route("/api/contexts/:id/entries", post(add_entry))
        .route("/api/search", get(search))
        .route("/api/status", get(status))
        .with_state(Arc::new(state));

    let listener = TcpListener::bind(&format!("127.0.0.1:{}", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn dashboard() -> Html<&'static str> {
    Html(besure::DASHBOARD_HTML)
}

async fn health() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse { ok: true, data: Some("ok"), error: None })
}

#[derive(Deserialize)]
struct AuthBody { password: String }

async fn auth(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AuthBody>,
) -> Json<ApiResponse<serde_json::Value>> {
    let vault = match besure::storage::Vault::open(Some(state.vault_root.clone())) {
        Ok(v) => v,
        Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("vault error".into()) }),
    };

    if !vault.config.encryption {
        let token = gen_token();
        state.sessions.lock().await.insert(token.clone());
        return Json(ApiResponse { ok: true, data: Some(serde_json::json!({"token": token})), error: None });
    }

    let salt = match vault.config.salt.as_ref() {
        Some(s) => s,
        None => return Json(ApiResponse { ok: false, data: None, error: Some("config error".into()) }),
    };
    let verify = match vault.config.verify_token.as_ref() {
        Some(v) => v,
        None => return Json(ApiResponse { ok: false, data: None, error: Some("config error".into()) }),
    };

    let mut crypto = besure::crypto::VaultCrypto::from_salt(salt.clone());
    let ok = crypto.unlock_with_verify(&body.password, verify).unwrap_or(false);

    if !ok {
        return Json(ApiResponse { ok: false, data: None, error: Some("Wrong password".into()) });
    }

    let token = gen_token();
    state.sessions.lock().await.insert(token.clone());
    Json(ApiResponse { ok: true, data: Some(serde_json::json!({"token": token})), error: None })
}

async fn auth_check(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Json<ApiResponse<bool>> {
    let sessions = state.sessions.lock().await;
    let authed = check_session(&headers, &sessions);
    Json(ApiResponse { ok: authed, data: Some(authed), error: None })
}

async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Json<ApiResponse<()>> {
    if let Some(auth) = headers.get("authorization") {
        if let Ok(s) = auth.to_str() {
            let token = s.trim_start_matches("Bearer ").trim();
            state.sessions.lock().await.remove(token);
        }
    }
    Json(ApiResponse { ok: true, data: Some(()), error: None })
}

fn check_session(headers: &HeaderMap, sessions: &HashSet<String>) -> bool {
    if let Some(auth) = headers.get("authorization") {
        if let Ok(s) = auth.to_str() {
            let token = s.trim_start_matches("Bearer ").trim();
            if sessions.contains(token) { return true; }
        }
    }
    false
}

fn gen_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

async fn list_contexts(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let sessions = state.sessions.lock().await;
    if !check_session(&headers, &sessions) {
        return Json(ApiResponse { ok: false, data: None, error: Some("Unauthorized".into()) });
    }
    drop(sessions);

    let vault = match besure::storage::Vault::open(Some(state.vault_root.clone())) {
        Ok(v) => v, Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("error".into()) }),
    };
    let db = match vault.database() { Ok(d) => d, Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("error".into()) }) };
    let ctxs = db.list_contexts().unwrap_or_default();
    let data: Vec<serde_json::Value> = ctxs.iter().map(|c| serde_json::to_value(c).unwrap_or_default()).collect();
    Json(ApiResponse { ok: true, data: Some(data), error: None })
}

#[derive(Deserialize)]
struct CreateBody { title: String, #[serde(default)] tags: Vec<String>, #[serde(default)] summary: String }

async fn create_context(State(state): State<Arc<AppState>>, headers: HeaderMap, Json(body): Json<CreateBody>) -> Json<ApiResponse<()>> {
    let sessions = state.sessions.lock().await;
    if !check_session(&headers, &sessions) {
        return Json(ApiResponse { ok: false, data: None, error: Some("Unauthorized".into()) });
    }
    drop(sessions);

    let mut vault = match besure::storage::Vault::open(Some(state.vault_root.clone())) {
        Ok(v) => v, Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("error".into()) }),
    };
    let mut ctx = besure::storage::Context::from_title(&body.title);
    ctx.tags = body.tags;
    ctx.summary = body.summary;
    if let Ok(db) = vault.database() {
        let _ = db.upsert_context(&ctx);
        let _ = vault.write_context_md(&ctx);
        let _ = vault.set_current(&ctx.id);
        let entry = besure::storage::Entry::new(&ctx.id, &format!("Context initialized: {}", ctx.title), "init");
        if let Ok(db) = vault.database() {
            let _ = db.add_entry(&entry);
        }
    }
    Json(ApiResponse { ok: true, data: Some(()), error: None })
}

async fn get_log(State(state): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<String>) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let sessions = state.sessions.lock().await;
    if !check_session(&headers, &sessions) {
        return Json(ApiResponse { ok: false, data: None, error: Some("Unauthorized".into()) });
    }
    drop(sessions);

    let vault = match besure::storage::Vault::open(Some(state.vault_root.clone())) {
        Ok(v) => v, Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("error".into()) }),
    };
    let db = match vault.database() { Ok(d) => d, Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("error".into()) }) };
    let entries = db.list_entries(&id).unwrap_or_default();
    let data: Vec<serde_json::Value> = entries.iter().map(|e| serde_json::to_value(e).unwrap_or_default()).collect();
    Json(ApiResponse { ok: true, data: Some(data), error: None })
}

#[derive(Deserialize)]
struct AddEntryBody { content: String, #[serde(default)] entry_type: String }

async fn add_entry(State(state): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<AddEntryBody>) -> Json<ApiResponse<()>> {
    let sessions = state.sessions.lock().await;
    if !check_session(&headers, &sessions) {
        return Json(ApiResponse { ok: false, data: None, error: Some("Unauthorized".into()) });
    }
    drop(sessions);

    let et = if body.entry_type.is_empty() { "progress".to_string() } else { body.entry_type };
    let vault = match besure::storage::Vault::open(Some(state.vault_root.clone())) {
        Ok(v) => v, Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("error".into()) }),
    };
    let entry = besure::storage::Entry::new(&id, &body.content, &et);
    if let Ok(db) = vault.database() {
        let _ = db.add_entry(&entry);
        let _ = vault.write_entry_md(&entry);
    }
    Json(ApiResponse { ok: true, data: Some(()), error: None })
}

#[derive(Deserialize)]
struct SearchQuery { q: String }

async fn search(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(query): Query<SearchQuery>) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let sessions = state.sessions.lock().await;
    if !check_session(&headers, &sessions) {
        return Json(ApiResponse { ok: false, data: None, error: Some("Unauthorized".into()) });
    }
    drop(sessions);

    let vault = match besure::storage::Vault::open(Some(state.vault_root.clone())) {
        Ok(v) => v, Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("error".into()) }),
    };
    let db = match vault.database() { Ok(d) => d, Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("error".into()) }) };
    let results = db.search(&query.q).unwrap_or_default();
    let data: Vec<serde_json::Value> = results.iter().map(|(c, e)| serde_json::json!({
        "context": {"id": c.id, "title": c.title, "status": c.status.to_string()},
        "entry": {"id": e.id, "date": e.date, "entry_type": e.entry_type, "content": e.content}
    })).collect();
    Json(ApiResponse { ok: true, data: Some(data), error: None })
}

async fn status(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Json<ApiResponse<serde_json::Value>> {
    let sessions = state.sessions.lock().await;
    if !check_session(&headers, &sessions) {
        return Json(ApiResponse { ok: false, data: None, error: Some("Unauthorized".into()) });
    }
    drop(sessions);

    let vault = match besure::storage::Vault::open(Some(state.vault_root.clone())) {
        Ok(v) => v, Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("error".into()) }),
    };
    let db = match vault.database() { Ok(d) => d, Err(_) => return Json(ApiResponse { ok: false, data: None, error: Some("error".into()) }) };
    let ctx_count = db.count_contexts().unwrap_or(0);
    let entry_count = db.count_entries().unwrap_or(0);
    Json(ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "contexts": ctx_count,
            "entries": entry_count,
            "current": vault.current_context,
            "encrypted": vault.config.encryption,
        })),
        error: None,
    })
}
