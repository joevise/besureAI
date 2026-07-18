use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use tokio::net::TcpListener;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::storage::{Vault, Context, Entry};
use crate::dashboard::DASHBOARD_HTML;

/// REST API Server (axum)
pub struct ApiServer {
    port: u16,
}

#[derive(Clone)]
struct AppState {
    vault_root: std::path::PathBuf,
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

impl ApiServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn run(&self) -> Result<()> {
        let state = AppState {
            vault_root: Vault::default_root(),
        };

        let app = Router::new()
            // Dashboard UI
            .route("/", get(dashboard))
            .route("/index.html", get(dashboard))
            // 健康检查
            .route("/api/health", get(health))
            // 上下文 CRUD
            .route("/api/contexts", get(list_contexts))
            .route("/api/contexts", post(create_context))
            .route("/api/contexts/:id", get(get_context))
            .route("/api/contexts/:id/entries", post(add_entry))
            .route("/api/contexts/:id/log", get(get_log))
            .route("/api/contexts/:id/export", get(export_context))
            // 搜索
            .route("/api/search", get(search))
            // 状态
            .route("/api/status", get(status))
            .with_state(Arc::new(state));

        println!(" Besure REST API on http://localhost:{}", self.port);
        let listener = TcpListener::bind(&format!("0.0.0.0:{}", self.port)).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

fn open_vault(state: &AppState) -> Result<Vault, (StatusCode, String)> {
    Vault::open(Some(state.vault_root.clone()))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

fn open_db(vault: &Vault) -> Result<crate::storage::db::Database, (StatusCode, String)> {
    vault.database()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn dashboard() -> axum::response::Html<&'static str> {
    axum::response::Html(DASHBOARD_HTML)
}

async fn health() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse { ok: true, data: Some("ok"), error: None })
}

async fn list_contexts(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<Context>>>, (StatusCode, String)> {
    let vault = open_vault(&state)?;
    let db = open_db(&vault)?;
    let contexts = db.list_contexts().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(contexts), error: None }))
}

async fn get_context(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Context>>, (StatusCode, String)> {
    let vault = open_vault(&state)?;
    let db = open_db(&vault)?;
    let ctx = db.get_context(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match ctx {
        Some(c) => Ok(Json(ApiResponse { ok: true, data: Some(c), error: None })),
        None => Err((StatusCode::NOT_FOUND, format!("context '{}' not found", id))),
    }
}

async fn create_context(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<Json<ApiResponse<Context>>, (StatusCode, String)> {
    let mut vault = open_vault(&state)?;

    let mut ctx = Context::from_title(&body.title);
    ctx.tags = body.tags;
    ctx.summary = body.summary;

    let db = open_db(&vault)?;
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
    Path(id): Path<String>,
    Json(body): Json<AddEntryBody>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, String)> {
    let vault = open_vault(&state)?;
    let entry_type = if body.entry_type.is_empty() { "progress".to_string() } else { body.entry_type };
    let entry = Entry::new(&id, &body.content, &entry_type);

    let db = vault.database().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db.add_entry(&entry).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    vault.write_entry_md(&entry).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ApiResponse { ok: true, data: Some(()), error: None }))
}

async fn get_log(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Vec<Entry>>>, (StatusCode, String)> {
    let vault = open_vault(&state)?;
    let db = open_db(&vault)?;
    let entries = db.list_entries(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ApiResponse { ok: true, data: Some(entries), error: None }))
}

async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, (StatusCode, String)> {
    let vault = open_vault(&state)?;
    let db = open_db(&vault)?;
    let results = db.search(&query.q).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let data: Vec<serde_json::Value> = results.iter().map(|(ctx, entry)| {
        serde_json::json!({
            "context": {
                "id": ctx.id,
                "title": ctx.title,
                "status": ctx.status.to_string(),
            },
            "entry": {
                "id": entry.id,
                "date": entry.date,
                "entry_type": entry.entry_type,
                "content": entry.content,
            }
        })
    }).collect();

    Ok(Json(ApiResponse { ok: true, data: Some(data), error: None }))
}

async fn status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, String)> {
    let vault = open_vault(&state)?;
    let db = open_db(&vault)?;
    let ctx_count = db.count_contexts().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let entry_count = db.count_entries().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let data = serde_json::json!({
        "contexts": ctx_count,
        "entries": entry_count,
        "current": vault.current_context,
        "vault_path": vault.root.display().to_string(),
    });

    Ok(Json(ApiResponse { ok: true, data: Some(data), error: None }))
}

async fn export_context(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<axum::response::Response<String>, (StatusCode, String)> {
    let vault = open_vault(&state)?;
    let db = open_db(&vault)?;
    let ctx = db.get_context(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, format!("context '{}' not found", id)))?;
    let entries = db.list_entries(&id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut md = format!("# {}\n\n", ctx.title);
    if !ctx.summary.is_empty() {
        md.push_str(&format!("## 背景\n{}\n\n", ctx.summary));
    }
    md.push_str(&format!("## 当前状态\n- 状态: {}\n- 创建: {}\n- 更新: {}\n\n", ctx.status, ctx.created, ctx.updated));
    md.push_str("## 进展时间线\n\n");
    for entry in entries.iter().rev() {
        md.push_str(&format!("### {} ({})\n{}\n\n", entry.date, entry.entry_type, entry.content));
    }
    md.push_str("---\n*Exported from Besure AI — 貔貅记忆*\n");

    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/markdown; charset=utf-8")
        .header("Content-Disposition", format!("attachment; filename=\"{}.md\"", ctx.id))
        .body(md)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?)
}
