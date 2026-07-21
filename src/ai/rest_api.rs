use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, Json, Redirect},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashSet;
use tokio::sync::Mutex;
use tokio::net::TcpListener;

use crate::storage::{Vault, Context, Entry, QueryFilter, Stats};
use crate::dashboard::DASHBOARD_HTML;

/// REST API + Dashboard Server
pub struct ApiServer {
    port: u16,
}

#[derive(Clone)]
struct AppState {
    vault_root: std::path::PathBuf,
    sessions: Arc<Mutex<HashSet<String>>>,  // active session tokens
}

#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

#[derive(Deserialize)]
struct CreateBody {
    title: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    summary: String,
}

#[derive(Deserialize)]
struct AddEntryBody {
    content: String,
    #[serde(default)]
    entry_type: String,
}

#[derive(Deserialize)]
struct AuthBody {
    password: String,
}

#[derive(Deserialize)]
struct QueryParams {
    context_id: Option<String>,
    all: Option<bool>,
    from_date: Option<String>,
    to_date: Option<String>,
    last_days: Option<i64>,
    /// Comma-separated entry types
    entry_types: Option<String>,
    keyword: Option<String>,
    resolved: Option<bool>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct AppendBody {
    content: String,
}

impl ApiServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let state = AppState {
            vault_root: Vault::default_root(),
            sessions: Arc::new(Mutex::new(HashSet::new())),
        };

        let app = Router::new()
            // Dashboard UI
            .route("/", get(dashboard))
            .route("/index.html", get(dashboard))
            // Auth
            .route("/api/auth", post(authenticate))
            .route("/api/auth/check", get(check_auth))
            .route("/api/auth/logout", post(logout))
            // 健康检查（不需要认证）
            .route("/api/health", get(health))
            // 以下需要认证
            .route("/api/contexts", get(list_contexts))
            .route("/api/contexts", post(create_context))
            .route("/api/contexts/:id", get(get_context))
            .route("/api/contexts/:id/entries", post(add_entry))
            .route("/api/contexts/:id/log", get(get_log))
            .route("/api/contexts/:id/export", get(export_context))
            .route("/api/search", get(search))
            .route("/api/status", get(status))
            .route("/api/query", get(query_entries))
            .route("/api/entries/:id/resolve", post(resolve_entry))
            .route("/api/entries/:id/append", post(append_entry))
            .route("/api/stats", get(get_stats_handler))
            .route("/api/tags", get(list_tags))
            // V0.5.5: multi-vault routes
            .route("/api/vaults", get(list_all_vaults))
            .route("/api/vaults/:id/contexts", get(get_vault_contexts))
            .route("/api/vaults/:id/log", get(get_vault_log))
            .route("/api/vaults/:id/stats", get(get_vault_stats))
            .route("/api/vaults/:id/unlock", post(unlock_vault))
            .with_state(Arc::new(state));

        let addr = format!("0.0.0.0:{}", self.port);
        println!(" Besure Dashboard on http://localhost:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

// === Auth Helpers ===

fn check_session(headers: &HeaderMap, sessions: &HashSet<String>) -> bool {
    if let Some(auth) = headers.get("authorization") {
        if let Ok(s) = auth.to_str() {
            let token = s.trim_start_matches("Bearer ").trim();
            return sessions.contains(token);
        }
    }
    // 也支持 cookie
    if let Some(cookie) = headers.get("cookie") {
        if let Ok(s) = cookie.to_str() {
            for c in s.split(';') {
                let c = c.trim();
                if let Some(token) = c.strip_prefix("besure_session=") {
                    if sessions.contains(token) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn generate_session_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::thread_rng().gen();
    hex_encode(&bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// === Auth Handlers ===

async fn authenticate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AuthBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, String)> {
    // 打开 vault 验证密码
    let vault = Vault::open(Some(state.vault_root.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !vault.config.encryption {
        // 无加密模式，直接通过
        let token = generate_session_token();
        state.sessions.lock().await.insert(token.clone());
        return Ok(Json(ApiResponse {
            ok: true,
            data: Some(serde_json::json!({"token": token})),
            error: None,
        }));
    }

    let salt = vault.config.salt.as_ref()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "no salt".to_string()))?;
    let verify = vault.config.verify_token.as_ref()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "no verify token".to_string()))?;

    let mut crypto = crate::crypto::VaultCrypto::from_salt(salt.clone());
    let ok = crypto.unlock_with_verify(&body.password, verify)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !ok {
        return Ok(Json(ApiResponse {
            ok: false,
            data: None,
            error: Some("密码错误".to_string()),
        }));
    }

    let token = generate_session_token();
    state.sessions.lock().await.insert(token.clone());

    Ok(Json(ApiResponse {
        ok: true,
        data: Some(serde_json::json!({"token": token})),
        error: None,
    }))
}

async fn check_auth(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<ApiResponse<bool>> {
    let sessions = state.sessions.lock().await;
    let authed = check_session(&headers, &sessions);
    Json(ApiResponse { ok: authed, data: Some(authed), error: None })
}

async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<ApiResponse<()>> {
    if let Some(auth) = headers.get("authorization") {
        if let Ok(s) = auth.to_str() {
            let token = s.trim_start_matches("Bearer ").trim();
            state.sessions.lock().await.remove(token);
        }
    }
    Json(ApiResponse { ok: true, data: Some(()), error: None })
}

// === Middleware-style auth check ===

fn require_auth(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<(), (StatusCode, Json<ApiResponse<()>>)> {
    let sessions = state.sessions.try_lock();
    match sessions {
        Ok(sessions) => {
            if check_session(headers, &sessions) {
                Ok(())
            } else {
                Err((StatusCode::UNAUTHORIZED, Json(ApiResponse {
                    ok: false,
                    data: None,
                    error: Some("未认证，请先登录".to_string()),
                })))
            }
        }
        Err(_) => Err((StatusCode::SERVICE_UNAVAILABLE, Json(ApiResponse {
            ok: false,
            data: None,
            error: Some("服务繁忙".to_string()),
        }))),
    }
}

// === Handlers ===

async fn dashboard() -> axum::response::Response<String> {
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Cache-Control", "no-cache, no-store, must-revalidate")
        .header("Pragma", "no-cache")
        .body(DASHBOARD_HTML.to_string())
        .unwrap()
}

async fn health() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse { ok: true, data: Some("ok"), error: None })
}

async fn list_contexts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<Context>>>, (StatusCode, Json<ApiResponse<()>>)> {
    require_auth(&headers, &state)?;
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse { ok: false, data: None, error: Some("vault error".into()) })))?;
    let db = vault.database().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse { ok: false, data: None, error: Some("db error".into()) })))?;
    let contexts = db.list_contexts().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse { ok: false, data: None, error: Some("query error".into()) })))?;
    Ok(Json(ApiResponse { ok: true, data: Some(contexts), error: None }))
}

async fn get_context(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Context>>, (StatusCode, String)> {
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let ctx = db.get_context(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match ctx {
        Some(c) => Ok(Json(ApiResponse { ok: true, data: Some(c), error: None })),
        None => Err((StatusCode::NOT_FOUND, format!("context '{}' not found", id))),
    }
}

async fn create_context(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> Result<Json<ApiResponse<Context>>, (StatusCode, String)> {
    let mut vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut ctx = Context::from_title(&body.title);
    ctx.tags = body.tags;
    ctx.summary = body.summary;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.upsert_context(&ctx).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    vault.write_context_md(&ctx).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    vault.set_current(&ctx.id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let entry = Entry::new(&ctx.id, &format!("上下文初始化: {}", ctx.title), "init");
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.add_entry(&entry).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(ctx), error: None }))
}

async fn add_entry(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<AddEntryBody>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let entry_type = if body.entry_type.is_empty() { "progress".to_string() } else { body.entry_type };
    let entry = Entry::new(&id, &body.content, &entry_type);
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 自动打标（同步，LLM 不可用时降级为空标签）
    let tagger = crate::ai::Tagger::from_app_config();
    let existing = db.list_vocab_tags().unwrap_or_default();
    let tags = tagger.tag(&body.content, &existing).unwrap_or_default();

    let mut entry = entry;
    entry.tags = tags.clone();
    db.add_entry(&entry).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !tags.is_empty() {
        let _ = db.bump_tags(&tags);
    }
    vault.write_entry_md(&entry).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn get_log(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Entry>>>, (StatusCode, String)> {
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let entries = db.list_entries(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(entries), error: None }))
}

async fn search(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, (StatusCode, String)> {
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let results = db.search(&query.q).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let data: Vec<serde_json::Value> = results.iter().map(|(ctx, entry)| {
        serde_json::json!({
            "context": {"id": ctx.id, "title": ctx.title, "status": ctx.status.to_string()},
            "entry": {"id": entry.id, "date": entry.date, "entry_type": entry.entry_type, "content": entry.content}
        })
    }).collect();
    Ok(Json(ApiResponse { ok: true, data: Some(data), error: None }))
}

async fn status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, String)> {
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let ctx_count = db.count_contexts().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let entry_count = db.count_entries().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "contexts": ctx_count,
            "entries": entry_count,
            "current": vault.current_context,
            "vault_path": vault.root.display().to_string(),
            "encrypted": vault.config.encryption,
        })),
        error: None,
    }))
}

async fn export_context(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<axum::response::Response<String>, (StatusCode, String)> {
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let ctx = db.get_context(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, format!("not found")))?;
    let entries = db.list_entries(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut md = format!("# {}\n\n## 进展时间线\n\n", ctx.title);
    for entry in entries.iter().rev() {
        md.push_str(&format!("### {} ({})\n{}\n\n", entry.date, entry.entry_type, entry.content));
    }
    md.push_str("---\n*Exported from Besure AI*");
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/markdown; charset=utf-8")
        .body(md)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?)
}

// === V0.4 new handlers ===

async fn query_entries(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<Vec<Entry>>>, (StatusCode, String)> {
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let all = params.all.unwrap_or(false);
    let context_id = if all {
        None
    } else if let Some(ref cid) = params.context_id {
        Some(cid.clone())
    } else {
        Some(vault.current_context.as_ref()
            .ok_or((StatusCode::BAD_REQUEST, "No active context. Provide context_id or all=true".to_string()))?
            .clone())
    };

    let from_date = if let Some(days) = params.last_days {
        Some((chrono::Utc::now() - chrono::Duration::days(days))
            .format("%Y-%m-%d")
            .to_string())
    } else {
        params.from_date.clone()
    };

    let entry_types: Vec<String> = params.entry_types
        .as_ref()
        .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
        .unwrap_or_default();

    let filter = QueryFilter {
        context_id,
        all_contexts: all,
        from_date,
        to_date: params.to_date.clone(),
        entry_types,
        keyword: params.keyword.clone(),
        resolved: params.resolved,
        limit: params.limit.unwrap_or(20),
    };

    let entries = db.query_entries(&filter).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(entries), error: None }))
}

async fn resolve_entry(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.update_entry_resolved(&id, true).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn append_entry(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<AppendBody>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.append_entry_content(&id, &body.content).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn get_stats_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Stats>>, (StatusCode, String)> {
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let stats = db.get_stats().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(stats), error: None }))
}

async fn list_tags(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, (StatusCode, String)> {
    require_auth(&headers, &state).map_err(|(code, _)| (code, "unauthorized".to_string()))?;
    let vault = Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let stats = db.get_vocab_stats().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let data: Vec<serde_json::Value> = stats.iter()
        .map(|(tag, count)| serde_json::json!({"tag": tag, "count": count}))
        .collect();
    Ok(Json(ApiResponse { ok: true, data: Some(data), error: None }))
}

// === V0.5.5: Multi-vault handlers ===

async fn list_all_vaults(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::storage::VaultInfo>>>, (StatusCode, String)> {
    let infos = crate::storage::Vault::list_all_vaults_info();
    Ok(Json(ApiResponse { ok: true, data: Some(infos), error: None }))
}

async fn get_vault_contexts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Context>>>, (StatusCode, String)> {
    let parent = crate::storage::Vault::vault_parent();
    let vault_path = parent.join(&id);
    if !vault_path.exists() {
        return Err((StatusCode::NOT_FOUND, format!("vault '{}' not found", id)));
    }
    let vault = Vault::open(Some(vault_path)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let contexts = db.list_contexts().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(contexts), error: None }))
}

async fn get_vault_log(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Entry>>>, (StatusCode, String)> {
    let parent = crate::storage::Vault::vault_parent();
    let vault_path = parent.join(&id);
    let vault = Vault::open(Some(vault_path)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Get entries from all contexts in this vault
    let contexts = db.list_contexts().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut all_entries = Vec::new();
    for ctx in &contexts {
        if let Ok(entries) = db.list_entries(&ctx.id) {
            all_entries.extend(entries);
        }
    }
    // Sort by date desc
    all_entries.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(Json(ApiResponse { ok: true, data: Some(all_entries), error: None }))
}

async fn get_vault_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Stats>>, (StatusCode, String)> {
    let parent = crate::storage::Vault::vault_parent();
    let vault_path = parent.join(&id);
    let vault = Vault::open(Some(vault_path)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let stats = db.get_stats().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(stats), error: None }))
}

#[derive(Deserialize)]
struct UnlockVaultBody {
    password: String,
}

async fn unlock_vault(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UnlockVaultBody>,
) -> Result<Json<ApiResponse<bool>>, (StatusCode, String)> {
    let parent = crate::storage::Vault::vault_parent();
    let vault_path = parent.join(&id);
    let mut vault = Vault::open(Some(vault_path)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !vault.config.encryption {
        return Ok(Json(ApiResponse { ok: true, data: Some(true), error: None }));
    }
    match vault.unlock(&body.password) {
        Ok(true) => Ok(Json(ApiResponse { ok: true, data: Some(true), error: None })),
        _ => Ok(Json(ApiResponse { ok: false, data: Some(false), error: Some("Wrong password".to_string()) })),
    }
}
