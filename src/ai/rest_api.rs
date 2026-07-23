use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, Json, Redirect},
    routing::{delete, get, post},
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
    #[serde(default)]
    semantic: Option<bool>,
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
            .route("/api/contexts/:id", get(get_context).delete(soft_delete_context_api))
            .route("/api/contexts/:id/restore", post(restore_context_api))
            .route("/api/contexts/:id/purge", delete(purge_context_api))
            .route("/api/entries/:id", delete(soft_delete_entry_api))
            .route("/api/entries/:id/restore", post(restore_entry_api))
            .route("/api/entries/:id/purge", delete(purge_entry_api))
            .route("/api/trash", get(list_trash_api))
            .route("/api/contexts/:id/entries", post(add_entry))
            .route("/api/contexts/:id/log", get(get_log))
            .route("/api/search", get(search))
            .route("/api/vaults/:id/search", get(vault_search))
            .route("/api/status", get(status))
            .route("/api/query", get(query_entries))
            .route("/api/entries/:id/resolve", post(resolve_entry))
            .route("/api/entries/:id/append", post(append_entry))
            .route("/api/stats", get(get_stats_handler))
            .route("/api/tags", get(list_tags))
            .route("/api/vaults", get(list_all_vaults))
            .route("/api/vaults/:id/contexts", get(get_vault_contexts))
            .route("/api/vaults/:id/contexts/:ctxId/stats", get(get_context_stats_handler))
            .route("/api/vaults/:id/log", get(get_vault_log))
            .route("/api/vaults/:id/stats", get(get_vault_stats))
            .route("/api/vaults/:id/unlock", post(unlock_vault))
            .route("/api/vaults/:id/contexts/:ctxId/export", get(export_context_encrypted))
            .route("/api/vaults/:id/import", post(import_vault))
            .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
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
    headers: HeaderMap,
    Json(body): Json<AuthBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, String)> {
    // 优先校验 Dashboard 独立密码（环境变量 BESURE_DASHBOARD_PASSWORD），与 vault 加密无关
    // 这解决了「vault 不加密 → 任何密码都能登录」的安全问题
    if let Ok(dash_pw) = std::env::var("BESURE_DASHBOARD_PASSWORD") {
        if !dash_pw.is_empty() {
            if body.password == dash_pw {
                let token = generate_session_token();
                state.sessions.lock().await.insert(token.clone());
                return Ok(Json(ApiResponse {
                    ok: true,
                    data: Some(serde_json::json!({"token": token})),
                    error: None,
                }));
            } else {
                return Ok(Json(ApiResponse {
                    ok: false,
                    data: None,
                    error: Some("wrong password".into()),
                }));
            }
        }
    }

    // 没有 Dashboard 独立密码时，走 vault 加密校验
    let vault = open_vault_from_headers(&headers, &state)?;

    if !vault.config.encryption {
        // 无加密且没设 Dashboard 密码：仅允许本机访问（生产环境应设密码）
        // 为了安全，无密码模式下也要求输入 vault 的 master password 作为确认
        // 但由于 vault 未加密无法验证，所以放行但打印警告
        eprintln!("⚠️  Dashboard: no encryption and no BESURE_DASHBOARD_PASSWORD set — accepting all passwords (insecure!)");
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

/// Open the correct vault based on X-Vault-Id header (multi-vault mode) or fallback to state.vault_root
fn open_vault_from_headers(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<Vault, (StatusCode, String)> {
    // Check for X-Vault-Id header (set by Dashboard frontend)
    if let Some(vault_id) = headers.get("x-vault-id").and_then(|v| v.to_str().ok()) {
        if !vault_id.is_empty() && vault_id != "default" {
            let parent = crate::storage::Vault::vault_parent();
            let vault_path = parent.join(vault_id);
            if !vault_path.exists() {
                return Err((StatusCode::NOT_FOUND, format!("vault '{}' not found", vault_id)));
            }
            return Vault::open(Some(vault_path)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
    }
    // Fallback: default vault root (backward compat for CLI/single-vault mode)
    Vault::open(Some(state.vault_root.clone())).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
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
    let vault = open_vault_from_headers(&headers, &state).map_err(|(c, m)| (c, Json(ApiResponse { ok: false, data: None, error: Some(m) })))?;
    let db = vault.database().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse { ok: false, data: None, error: Some("db error".into()) })))?;
    let contexts = db.list_contexts().map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse { ok: false, data: None, error: Some("query error".into()) })))?;
    Ok(Json(ApiResponse { ok: true, data: Some(contexts), error: None }))
}

async fn get_context(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Context>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
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
    let mut vault = open_vault_from_headers(&headers, &state)?;
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

// === Recycle Bin handlers ===

async fn soft_delete_context_api(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.soft_delete_context(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn soft_delete_entry_api(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.soft_delete_entry(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let vec_db_path = vault.root.join("vectors.db");
    if vec_db_path.exists() {
        if let Ok(store) = crate::ai::VectorStore::open(&vec_db_path) {
            let _ = store.delete_by_entry(&id);
        }
    }
    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn restore_context_api(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.restore_context(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn restore_entry_api(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.restore_entry(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn purge_context_api(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.hard_delete_context(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn purge_entry_api(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.hard_delete_entry(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let vec_db_path = vault.root.join("vectors.db");
    if vec_db_path.exists() {
        if let Ok(store) = crate::ai::VectorStore::open(&vec_db_path) {
            let _ = store.delete_by_entry(&id);
        }
    }
    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn list_trash_api(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (contexts, entries) = db.list_trash().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "contexts": contexts,
            "entries": entries,
        })),
        error: None,
    }))
}

async fn add_entry(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<AddEntryBody>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
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

    // 自动增量索引（失败降级，不阻塞 add）
    let vault_root = vault.root.clone();
    let entry_id = entry.id.clone();
    let entry_ctx = entry.context_id.clone();
    let entry_content = entry.content.clone();
    tokio::task::spawn_blocking(move || {
        let res: anyhow::Result<()> = (|| {
            let provider = crate::ai::EmbeddingProvider::from_app_config();
            let vec = provider.embed(&entry_content)?;
            let store = crate::ai::VectorStore::open(&vault_root.join("vectors.db"))?;
            store.upsert_embedding(&entry_id, &entry_ctx, Some(&entry_id), &entry_content, &vec)?;
            Ok(())
        })();
        if let Err(e) = res {
            eprintln!("⚠️  auto-index skipped: {}", e);
        }
    });

    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn get_log(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Entry>>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let entries = db.list_entries(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(entries), error: None }))
}

/// 语义搜索：embed query → vectors.db 余弦相似度 → 关联 entry/context
fn semantic_search_vault(vault: &Vault, q: &str) -> Result<Vec<serde_json::Value>, (StatusCode, String)> {
    let vec_db_path = vault.root.join("vectors.db");
    // 从当前 vault 目录读 appconfig（不依赖全局 BESURE_VAULT 环境变量）
    let cfg_path = vault.root.join("appconfig.json");
    let emb_config = std::fs::read_to_string(&cfg_path)
        .ok()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
        .and_then(|v| v.get("embedding").cloned())
        .and_then(|e| serde_json::from_value::<crate::ai::embedding::EmbeddingConfig>(e).ok())
        .unwrap_or_default();
    if !vec_db_path.exists() {
        // 自动建索引（首次语义搜索会变慢几秒，但比报错好）
        eprintln!("[info] vectors.db not found, building index automatically...");
        let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let entries = db.list_all_entries().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if !entries.is_empty() {
            let store = crate::ai::VectorStore::open(&vec_db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let provider = crate::ai::EmbeddingProvider::new(emb_config.clone());
            for entry in &entries {
                if !store.has_entry(&entry.id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))? {
                    let vec = provider.embed(&entry.content).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    store.upsert_embedding(&entry.id, &entry.context_id, Some(&entry.id), &entry.content, &vec)
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                }
            }
        }
    }
    let provider = crate::ai::EmbeddingProvider::new(emb_config);
    let query_vec = provider.embed(q).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let store = crate::ai::VectorStore::open(&vec_db_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let results = store.search(&query_vec, 10).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let data: Vec<serde_json::Value> = results.iter().map(|r| {
        let ctx_title = db.get_context(&r.context_id).ok().flatten()
            .map(|c| c.title).unwrap_or_else(|| r.context_id.clone());
        let entry_json = r.entry_id.as_deref()
            .and_then(|eid| db.get_entry(eid).ok().flatten())
            .map(|e| serde_json::json!({"id": e.id, "date": e.date, "entry_type": e.entry_type, "content": e.content}))
            .unwrap_or(serde_json::json!({"content": r.chunk_text}));
        serde_json::json!({
            "score": r.score,
            "context": {"id": r.context_id, "title": ctx_title},
            "entry": entry_json,
        })
    }).collect();
    Ok(data)
}

async fn search(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
    if query.semantic.unwrap_or(false) {
        let data = semantic_search_vault(&vault, &query.q)?;
        return Ok(Json(ApiResponse { ok: true, data: Some(data), error: None }));
    }
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

/// Vault-scoped search: GET /api/vaults/:id/search?q=xxx&semantic=true
async fn vault_search(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, (StatusCode, String)> {
    let parent = crate::storage::Vault::vault_parent();
    let vault_path = parent.join(&id);
    if !vault_path.exists() {
        return Err((StatusCode::NOT_FOUND, format!("vault '{}' not found", id)));
    }
    let vault = Vault::open(Some(vault_path)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if query.semantic.unwrap_or(false) {
        let data = semantic_search_vault(&vault, &query.q)?;
        return Ok(Json(ApiResponse { ok: true, data: Some(data), error: None }));
    }
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
    let vault = open_vault_from_headers(&headers, &state)?;
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

#[derive(Deserialize)]
struct ExportQuery {
    password: Option<String>,
}

/// Vault-scoped encrypted export: GET /api/vaults/:id/contexts/:ctxId/export?password=xxx
/// Returns .besure binary (AES-256-GCM encrypted JSON payload).
async fn export_context_encrypted(
    State(_state): State<Arc<AppState>>,
    axum::extract::Path((vault_id, ctx_id)): axum::extract::Path<(String, String)>,
    Query(query): Query<ExportQuery>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    let password = query.password
        .filter(|p| !p.is_empty())
        .ok_or((StatusCode::BAD_REQUEST, "missing password".to_string()))?;
    let parent = crate::storage::Vault::vault_parent();
    let vault_path = parent.join(&vault_id);
    if !vault_path.exists() {
        return Err((StatusCode::NOT_FOUND, format!("vault '{}' not found", vault_id)));
    }
    let vault = Vault::open(Some(vault_path)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (bytes, _count) = crate::export::export_bytes(&db, &ctx_id, &password)
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/octet-stream")
        .header("Content-Disposition", format!("attachment; filename=\"{}.besure\"", ctx_id))
        .body(axum::body::Body::from(bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

#[derive(Deserialize)]
struct ImportBody {
    password: String,
    file_base64: String,
    /// Optional: import all entries into this existing context instead of restoring the original
    target_context_id: Option<String>,
}

/// Vault-scoped import: POST /api/vaults/:id/import  body: {password, file_base64, target_context_id?}
async fn import_vault(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ImportBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, String)> {
    let parent = crate::storage::Vault::vault_parent();
    let vault_path = parent.join(&id);
    if !vault_path.exists() {
        return Err((StatusCode::NOT_FOUND, format!("vault '{}' not found", id)));
    }
    let data = crate::export::b64_decode(&body.file_base64)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid base64: {}", e)))?;
    let vault = Vault::open(Some(vault_path)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let result = crate::export::import_bytes(&db, &data, &body.password, body.target_context_id.as_deref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "context_id": result.context.id,
            "context_title": result.context.title,
            "entries_imported": result.entries_imported,
            "entries_skipped": result.entries_skipped,
        })),
        error: None,
    }))
}

// === V0.4 new handlers ===

async fn query_entries(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<Vec<Entry>>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
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
    let vault = open_vault_from_headers(&headers, &state)?;
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
    let vault = open_vault_from_headers(&headers, &state)?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.append_entry_content(&id, &body.content).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn get_stats_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Stats>>, (StatusCode, String)> {
    let vault = open_vault_from_headers(&headers, &state)?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let stats = db.get_stats().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(stats), error: None }))
}

/// Context 详情页 Stats（vault-scoped，含 by_tag，不含 by_context）
async fn get_context_stats_handler(
    State(_state): State<Arc<AppState>>,
    axum::extract::Path((vault_id, context_id)): axum::extract::Path<(String, String)>,
) -> Result<Json<ApiResponse<crate::storage::db::ContextStats>>, (StatusCode, String)> {
    let parent = crate::storage::Vault::vault_parent();
    let vault_path = parent.join(&vault_id);
    let vault = Vault::open(Some(vault_path)).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let stats = db.get_context_stats(&context_id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(stats), error: None }))
}

async fn list_tags(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, (StatusCode, String)> {
    require_auth(&headers, &state).map_err(|(code, _)| (code, "unauthorized".to_string()))?;
    let vault = open_vault_from_headers(&headers, &state)?;
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
