use anyhow::Result;
use anyhow::Context as _;  // trait, renamed to avoid clash with storage::Context
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

use crate::storage::{Vault, Entry};
use crate::storage::models::Context;

/// MCP Server (stdio JSON-RPC 2.0)
///
/// 实现 Model Context Protocol 的基本协议：
/// - initialize / initialized
/// - tools/list → 暴露 besure 操作
/// - tools/call → 执行 besure 命令
pub struct McpServer;

#[derive(Serialize, Deserialize)]
struct RpcRequest {
    jsonrpc: String,
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

impl McpServer {
    /// 启动 MCP Server（stdio）
    pub fn run() -> Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line.context("failed to read stdin")?;
            if line.trim().is_empty() {
                continue;
            }

            let req: RpcRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let response = Self::handle(&req);
            let output = serde_json::to_string(&response)? + "\n";
            stdout.write_all(output.as_bytes())?;
            stdout.flush()?;
        }

        Ok(())
    }

    fn handle(req: &RpcRequest) -> Value {
        match req.method.as_str() {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "besure",
                        "version": "0.61.0"
                    }
                }
            }),
            "notifications/initialized" => json!({}), // notification, no response
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {
                    "tools": Self::tool_definitions()
                }
            }),
            "tools/call" => Self::handle_tool_call(req),
            _ => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "error": {
                    "code": -32601,
                    "message": format!("method not found: {}", req.method)
                }
            }),
        }
    }

    fn tool_definitions() -> Vec<Value> {
        vec![
            json!({
                "name": "besure_list_contexts",
                "description": "列出所有上下文",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "status": {"type": "string", "description": "过滤状态: active/paused/completed/archived"}
                    }
                }
            }),
            json!({
                "name": "besure_get_context",
                "description": "加载完整上下文信息",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "上下文 ID"}
                    },
                    "required": ["id"]
                }
            }),
            json!({
                "name": "besure_get_status",
                "description": "查看某上下文或全局状态",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "上下文 ID（不传则返回全局状态）"}
                    }
                }
            }),
            json!({
                "name": "besure_add_entry",
                "description": "⚠️ MANDATORY: Record progress/decision/milestone/lesson to Besure memory. Call this after completing tasks, making decisions, or learning lessons. If in doubt, RECORD. Lost memory is worse than over-recording.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "上下文 ID"},
                        "content": {"type": "string", "description": "进展内容"},
                        "entry_type": {"type": "string", "description": "类型: init/milestone/decision/progress/blocker/note"}
                    },
                    "required": ["id", "content"]
                }
            }),
            json!({
                "name": "besure_search",
                "description": "搜索所有上下文。semantic=false 全文匹配（默认）；semantic=true 本地语义搜索（bge-small-zh，离线向量），可命中语义相关但不含关键词的记忆。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "搜索关键词或语义描述"},
                        "semantic": {"type": "boolean", "description": "true=本地语义向量搜索，false=全文匹配（默认）"}
                    },
                    "required": ["query"]
                }
            }),
            json!({
                "name": "besure_create",
                "description": "创建新上下文",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string", "description": "上下文标题"},
                        "tags": {"type": "array", "items": {"type": "string"}, "description": "标签列表"},
                        "summary": {"type": "string", "description": "摘要"}
                    },
                    "required": ["title"]
                }
            }),
            json!({
                "name": "besure_switch",
                "description": "切换到某上下文",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "上下文 ID 或关键词（支持模糊匹配）"}
                    },
                    "required": ["query"]
                }
            }),
            json!({
                "name": "besure_export",
                "description": "导出上下文。带 password 时导出为 AES-256-GCM 加密的 .besure（返回 base64）；不带 password 导出为 Markdown。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "上下文 ID"},
                        "output": {"type": "string", "description": "输出文件路径（可选）"},
                        "password": {"type": "string", "description": "加密密码（提供则导出加密 .besure）"}
                    },
                    "required": ["id"]
                }
            }),
            json!({
                "name": "besure_import",
                "description": "导入加密 .besure 文件（base64 内容 + 密码）到当前 vault，entry 按 id 去重",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_base64": {"type": "string", "description": ".besure 文件的 base64 内容"},
                        "password": {"type": "string", "description": "解密密码"}
                    },
                    "required": ["file_base64", "password"]
                }
            }),
            json!({
                "name": "besure_link",
                "description": "给 entry 建立关联（因果/替代/引用等）",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "entry_id": {"type": "string", "description": "源 entry ID"},
                        "target_id": {"type": "string", "description": "目标 entry/context ID"},
                        "relation": {"type": "string", "description": "关系: caused_by/supersedes/related_to/ref_file/ref_commit/ref_url"}
                    },
                    "required": ["entry_id", "target_id"]
                }
            }),
            json!({
                "name": "besure_expire",
                "description": "标记 entry 过期",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "entry_id": {"type": "string", "description": "entry ID"}
                    },
                    "required": ["entry_id"]
                }
            }),
            json!({
                "name": "besure_supersede",
                "description": "标记旧 entry 被新 entry 替代",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "old_id": {"type": "string", "description": "旧 entry ID"},
                        "new_id": {"type": "string", "description": "新 entry ID"}
                    },
                    "required": ["old_id", "new_id"]
                }
            }),
            json!({
                "name": "besure_recall",
                "description": "主动召回：返回即将过期/已过期/最近24h/被替代的记忆",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            json!({
                "name": "besure_query",
                "description": "统一查询 entries，支持时间/类型/上下文/关键词/resolved 过滤。默认当前上下文最近20条。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "context_id": {"type": "string", "description": "上下文 ID（不传则用当前）"},
                        "all": {"type": "boolean", "description": "搜索所有上下文"},
                        "from_date": {"type": "string", "description": "开始日期 YYYY-MM-DD"},
                        "to_date": {"type": "string", "description": "结束日期 YYYY-MM-DD"},
                        "last_days": {"type": "integer", "description": "最近N天"},
                        "entry_types": {"type": "array", "items": {"type": "string"}, "description": "类型过滤"},
                        "keyword": {"type": "string", "description": "关键词"},
                        "resolved": {"type": "boolean", "description": "resolved 过滤: true/false"},
                        "limit": {"type": "integer", "description": "返回条数，默认20"}
                    }
                }
            }),
            json!({
                "name": "besure_resolve",
                "description": "标记 entry 为 resolved",
                "inputSchema": {
                    "type": "object",
                    "properties": {"entry_id": {"type": "string"}},
                    "required": ["entry_id"]
                }
            }),
            json!({
                "name": "besure_append",
                "description": "追加内容到已有 entry",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "entry_id": {"type": "string"},
                        "content": {"type": "string", "description": "追加的内容"}
                    },
                    "required": ["entry_id", "content"]
                }
            }),
            json!({
                "name": "besure_stats",
                "description": "统计概览：按上下文/类型/状态/resolved 的分布",
                "inputSchema": {"type": "object", "properties": {}}
            }),
            json!({
                "name": "besure_vaults",
                "description": "列出所有 vault（需要 BESURE_VAULTS_ALL=true）",
                "inputSchema": {"type": "object", "properties": {}}
            }),
            json!({
                "name": "besure_share",
                "description": "推送 entry 到共享 vault",
                "inputSchema": {
                    "type": "object",
                    "properties": {"entry_id": {"type": "string"}},
                    "required": ["entry_id"]
                }
            }),
            json!({
                "name": "besure_list_tags",
                "description": "列出自动标签库（tag + 使用次数，按使用频率降序）",
                "inputSchema": {"type": "object", "properties": {}}
            }),
            json!({
                "name": "besure_delete_entry",
                "description": "把 entry 移入回收站（软删除，可恢复）",
                "inputSchema": {
                    "type": "object",
                    "properties": {"entry_id": {"type": "string", "description": "entry ID"}},
                    "required": ["entry_id"]
                }
            }),
            json!({
                "name": "besure_restore_entry",
                "description": "从回收站恢复 entry",
                "inputSchema": {
                    "type": "object",
                    "properties": {"entry_id": {"type": "string", "description": "entry ID"}},
                    "required": ["entry_id"]
                }
            }),
            json!({
                "name": "besure_shared",
                "description": "查看共享 vault 内容",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "keyword": {"type": "string"},
                        "limit": {"type": "integer", "description": "默认20"}
                    }
                }
            }),
        ]
    }

    fn handle_tool_call(req: &RpcRequest) -> Value {
        let tool_name = req.params
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let args = req.params.get("arguments").cloned().unwrap_or(json!({}));

        let result = match tool_name {
            "besure_list_contexts" => Self::tool_list_contexts(&args),
            "besure_get_context" => Self::tool_get_context(&args),
            "besure_get_status" => Self::tool_get_status(&args),
            "besure_add_entry" => Self::tool_add_entry(&args),
            "besure_search" => Self::tool_search(&args),
            "besure_create" => Self::tool_create(&args),
            "besure_switch" => Self::tool_switch(&args),
            "besure_export" => Self::tool_export(&args),
            "besure_import" => Self::tool_import(&args),
            "besure_link" => Self::tool_link(&args),
            "besure_expire" => Self::tool_expire(&args),
            "besure_supersede" => Self::tool_supersede(&args),
            "besure_recall" => Self::tool_recall(&args),
            "besure_query" => Self::tool_query(&args),
            "besure_resolve" => Self::tool_resolve(&args),
            "besure_append" => Self::tool_append(&args),
            "besure_stats" => Self::tool_stats(&args),
            "besure_vaults" => Self::tool_vaults(&args),
            "besure_share" => Self::tool_share(&args),
            "besure_shared" => Self::tool_shared(&args),
            "besure_delete_entry" => Self::tool_delete_entry(&args),
            "besure_restore_entry" => Self::tool_restore_entry(&args),
            "besure_list_tags" => Self::tool_list_tags(&args),
            _ => Err(format!("unknown tool: {}", tool_name)),
        };

        match result {
            Ok(text) => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {
                    "content": [{"type": "text", "text": text}]
                }
            }),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "result": {
                    "isError": true,
                    "content": [{"type": "text", "text": e}]
                }
            }),
        }
    }

    fn get_vault() -> Result<Vault, String> {
        if !Vault::exists(None) {
            return Err("vault not initialized".to_string());
        }
        Vault::open(None).map_err(|e| e.to_string())
    }

    fn tool_list_contexts(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let db = vault.database().map_err(|e| e.to_string())?;
        let contexts = db.list_contexts().map_err(|e| e.to_string())?;

        let status_filter = args.get("status").and_then(|s| s.as_str());
        let filtered: Vec<&Context> = match status_filter {
            Some(s) => contexts.iter().filter(|c| c.status.to_string() == s).collect(),
            None => contexts.iter().collect(),
        };

        if filtered.is_empty() {
            return Ok("No contexts found.".to_string());
        }

        let mut lines = Vec::new();
        for ctx in &filtered {
            let marker = if vault.current_context.as_deref() == Some(&ctx.id) { "▶" } else { " " };
            lines.push(format!("{} {} ({}) [{}]", marker, ctx.title, ctx.id, ctx.status));
        }
        Ok(lines.join("\n"))
    }

    fn tool_get_context(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let id = args.get("id").and_then(|i| i.as_str()).ok_or("missing 'id'")?;
        let db = vault.database().map_err(|e| e.to_string())?;

        let ctx = db.get_context(id).map_err(|e| e.to_string())?
            .ok_or(format!("context '{}' not found", id))?;

        let entries = db.list_entries(id).map_err(|e| e.to_string())?;

        let mut output = format!("═══ {} ({}) ═══\n", ctx.title, ctx.id);
        output.push_str(&format!("Status: {} | Created: {} | Updated: {}\n", ctx.status, ctx.created, ctx.updated));
        if !ctx.summary.is_empty() {
            output.push_str(&format!("Summary: {}\n", ctx.summary));
        }
        if !ctx.tags.is_empty() {
            output.push_str(&format!("Tags: {}\n", ctx.tags.join(", ")));
        }
        output.push_str(&format!("\nEntries ({}):\n", entries.len()));
        for (i, entry) in entries.iter().enumerate() {
            output.push_str(&format!("  [{}] ({}) {}\n", entries.len() - i, entry.entry_type, entry.content));
        }
        Ok(output)
    }

    fn tool_get_status(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let db = vault.database().map_err(|e| e.to_string())?;

        if let Some(id) = args.get("id").and_then(|i| i.as_str()) {
            let ctx = db.get_context(id).map_err(|e| e.to_string())?
                .ok_or(format!("context '{}' not found", id))?;
            Ok(format!("{} ({})\nStatus: {}\nUpdated: {}\nSummary: {}",
                ctx.title, ctx.id, ctx.status, ctx.updated, ctx.summary))
        } else {
            let ctx_count = db.count_contexts().map_err(|e| e.to_string())?;
            let entry_count = db.count_entries().map_err(|e| e.to_string())?;
            let current = vault.current_context.as_ref().map(|s| s.clone()).unwrap_or_default();
            Ok(format!("Besure AI — 貔貅记忆\nContexts: {}\nEntries: {}\nCurrent: {}",
                ctx_count, entry_count, current))
        }
    }

    fn tool_add_entry(args: &Value) -> Result<String, String> {
        let mut vault = Self::get_vault()?;
        let id = args.get("id").and_then(|i| i.as_str()).ok_or("missing 'id'")?;
        let content = args.get("content").and_then(|c| c.as_str()).ok_or("missing 'content'")?;
        let entry_type = args.get("entry_type").and_then(|t| t.as_str()).unwrap_or("progress");

        let entry = Entry::new(id, content, entry_type);
        let db = vault.database().map_err(|e| e.to_string())?;

        // 自动打标（同步，LLM 不可用时降级为空标签）
        let tagger = crate::ai::Tagger::from_app_config();
        let existing = db.list_vocab_tags().unwrap_or_default();
        let tags = tagger.tag(content, &existing).unwrap_or_default();

        let mut entry = entry;
        entry.tags = tags.clone();
        db.add_entry(&entry).map_err(|e| e.to_string())?;
        if !tags.is_empty() {
            let _ = db.bump_tags(&tags);
        }
        vault.write_entry_md(&entry).map_err(|e| e.to_string())?;

        // 自动增量索引（失败降级，不阻塞 add）
        if let Err(e) = Self::try_index_entry(&vault, &entry) {
            eprintln!("⚠️  auto-index skipped ({}): {}", entry.id, e);
        }

        if tags.is_empty() {
            Ok(format!("✓ Added {} entry to {}", entry_type, id))
        } else {
            Ok(format!("✓ Added {} entry to {}  🏷 {}", entry_type, id, tags.join(", ")))
        }
    }

    fn try_index_entry(vault: &Vault, entry: &Entry) -> anyhow::Result<()> {
        let provider = crate::ai::EmbeddingProvider::from_app_config();
        let vec = provider.embed(&entry.content)?;
        let store = crate::ai::VectorStore::open(&vault.root.join("vectors.db"))?;
        store.upsert_embedding(&entry.id, &entry.context_id, Some(&entry.id), &entry.content, &vec)?;
        Ok(())
    }

    fn tool_list_tags(_args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let db = vault.database().map_err(|e| e.to_string())?;
        let stats = db.get_vocab_stats().map_err(|e| e.to_string())?;

        if stats.is_empty() {
            return Ok("No tags yet.".to_string());
        }

        let lines: Vec<String> = stats.iter()
            .map(|(tag, count)| format!("  {} ({})", tag, count))
            .collect();
        Ok(format!("Tags ({}):\n{}", stats.len(), lines.join("\n")))
    }

    fn tool_search(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let query = args.get("query").and_then(|q| q.as_str()).ok_or("missing 'query'")?;
        let semantic = args.get("semantic").and_then(|s| s.as_bool()).unwrap_or(false);
        if semantic {
            return Self::tool_semantic_search(&vault, query);
        }
        let db = vault.database().map_err(|e| e.to_string())?;
        let results = db.search(query).map_err(|e| e.to_string())?;

        if results.is_empty() {
            return Ok(format!("No results for '{}'.", query));
        }

        let mut lines = Vec::new();
        let mut current_ctx = String::new();
        for (ctx, entry) in &results {
            if ctx.id != current_ctx {
                current_ctx = ctx.id.clone();
                lines.push(format!("─── {} ({}) ───", ctx.title, ctx.id));
            }
            lines.push(format!("  [{}] {} | {}", entry.date, entry.entry_type, entry.content));
        }
        lines.push(format!("\n{} results found.", results.len()));
        Ok(lines.join("\n"))
    }

    fn tool_semantic_search(vault: &Vault, query: &str) -> Result<String, String> {
        let vec_db_path = vault.root.join("vectors.db");
        if !vec_db_path.exists() {
            return Err("No vectors indexed yet. Run 'besure index --all' first.".to_string());
        }
        let provider = crate::ai::EmbeddingProvider::from_app_config();
        let query_vec = provider.embed(query).map_err(|e| e.to_string())?;
        let store = crate::ai::VectorStore::open(&vec_db_path).map_err(|e| e.to_string())?;
        let results = store.search(&query_vec, 10).map_err(|e| e.to_string())?;

        if results.is_empty() {
            return Ok(format!("No semantic results for '{}'.", query));
        }

        let db = vault.database().map_err(|e| e.to_string())?;
        let mut lines = vec![format!("Semantic search results for \"{}\":", query)];
        for r in &results {
            let meta = r.entry_id.as_deref()
                .and_then(|eid| db.get_entry(eid).ok().flatten())
                .map(|e| format!("{} | {}", e.date, e.entry_type))
                .unwrap_or_default();
            lines.push(format!("  [{:.3}] {} | {} | {}", r.score, r.context_id, meta,
                r.chunk_text.chars().take(80).collect::<String>()));
        }
        Ok(lines.join("\n"))
    }

    fn tool_create(args: &Value) -> Result<String, String> {
        let mut vault = Self::get_vault()?;
        let title = args.get("title").and_then(|t| t.as_str()).ok_or("missing 'title'")?;
        let tags: Vec<String> = args.get("tags")
            .and_then(|t| serde_json::from_value(t.clone()).ok())
            .unwrap_or_default();
        let summary = args.get("summary").and_then(|s| s.as_str()).unwrap_or("");

        let mut ctx = Context::from_title(title);
        ctx.tags = tags;
        ctx.summary = summary.to_string();

        let db = vault.database().map_err(|e| e.to_string())?;
        db.upsert_context(&ctx).map_err(|e| e.to_string())?;
        vault.write_context_md(&ctx).map_err(|e| e.to_string())?;
        vault.set_current(&ctx.id).map_err(|e| e.to_string())?;

        let entry = Entry::new(&ctx.id, &format!("上下文初始化: {}", ctx.title), "init");
        let db = vault.database().map_err(|e| e.to_string())?;
        db.add_entry(&entry).map_err(|e| e.to_string())?;

        Ok(format!("✓ Created context: {} ({})", ctx.title, ctx.id))
    }

    fn tool_switch(args: &Value) -> Result<String, String> {
        let mut vault = Self::get_vault()?;
        let query = args.get("query").and_then(|q| q.as_str()).ok_or("missing 'query'")?;
        let db = vault.database().map_err(|e| e.to_string())?;

        // 精确匹配
        if db.get_context(query).map_err(|e| e.to_string())?.is_some() {
            vault.set_current(query).map_err(|e| e.to_string())?;
            return Ok(format!("✓ Switched to: {}", query));
        }

        // 模糊匹配
        let found = db.find_contexts_fuzzy(query).map_err(|e| e.to_string())?;
        if found.len() == 1 {
            let ctx = &found[0];
            vault.set_current(&ctx.id).map_err(|e| e.to_string())?;
            Ok(format!("✓ Switched to: {} ({})", ctx.title, ctx.id))
        } else if found.is_empty() {
            Err(format!("No context found matching '{}'", query))
        } else {
            let names: Vec<String> = found.iter()
                .map(|c| format!("  {} ({})", c.title, c.id))
                .collect();
            Err(format!("Multiple matches:\n{}", names.join("\n")))
        }
    }

    fn tool_export(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let id = args.get("id").and_then(|i| i.as_str()).ok_or("missing 'id'")?;
        let db = vault.database().map_err(|e| e.to_string())?;

        if let Some(password) = args.get("password").and_then(|p| p.as_str()).filter(|p| !p.is_empty()) {
            // Encrypted .besure export → base64
            let (bytes, count) = crate::export::export_bytes(&db, id, password).map_err(|e| e.to_string())?;
            if let Some(output) = args.get("output").and_then(|o| o.as_str()) {
                std::fs::write(output, &bytes).map_err(|e| e.to_string())?;
                return Ok(format!("✓ Exported {} entries to {} (AES-256-GCM encrypted)\nbase64:\n{}", count, output, crate::export::b64_encode(&bytes)));
            }
            return Ok(format!("✓ Exported {} entries (AES-256-GCM encrypted .besure, base64):\n{}", count, crate::export::b64_encode(&bytes)));
        }

        // Legacy Markdown export
        let default_output = format!("{}.md", id);
        let output = args.get("output").and_then(|o| o.as_str())
            .unwrap_or(&default_output);

        let ctx = db.get_context(id).map_err(|e| e.to_string())?
            .ok_or(format!("context '{}' not found", id))?;
        let entries = db.list_entries(id).map_err(|e| e.to_string())?;

        let output_path = std::path::PathBuf::from(output);
        vault.export_context(&ctx, &entries, &output_path).map_err(|e| e.to_string())?;

        Ok(format!("✓ Exported '{}' to {} ({} entries)", ctx.title, output, entries.len()))
    }

    fn tool_import(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let file_base64 = args.get("file_base64").and_then(|s| s.as_str()).ok_or("missing 'file_base64'")?;
        let password = args.get("password").and_then(|p| p.as_str()).ok_or("missing 'password'")?;

        let data = crate::export::b64_decode(file_base64).map_err(|e| e.to_string())?;
        let db = vault.database().map_err(|e| e.to_string())?;
        let result = crate::export::import_bytes(&db, &data, password, None).map_err(|e| e.to_string())?;

        Ok(format!(
            "✓ Imported context '{}' ({}) — {} entries imported, {} skipped (already exist)",
            result.context.title, result.context.id, result.entries_imported, result.entries_skipped
        ))
    }

    fn tool_link(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let entry_id = args.get("entry_id").and_then(|i| i.as_str()).ok_or("missing 'entry_id'")?;
        let target_id = args.get("target_id").and_then(|i| i.as_str()).ok_or("missing 'target_id'")?;
        let relation_str = args.get("relation").and_then(|r| r.as_str()).unwrap_or("related_to");

        let relation = relation_str.parse()
            .map_err(|e: String| format!("invalid relation '{}': {}", relation_str, e))?;

        let link = crate::storage::models::EntryLink { target_id: target_id.to_string(), relation };
        let db = vault.database().map_err(|e| e.to_string())?;
        db.add_entry_link(entry_id, &link).map_err(|e| e.to_string())?;

        Ok(format!("✓ Linked {} → {} ({})", entry_id, target_id, relation_str))
    }

    fn tool_expire(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let entry_id = args.get("entry_id").and_then(|i| i.as_str()).ok_or("missing 'entry_id'")?;

        let db = vault.database().map_err(|e| e.to_string())?;
        let entry = db.get_entry(entry_id).map_err(|e| e.to_string())?
            .ok_or(format!("entry '{}' not found", entry_id))?;

        use crate::storage::models::EntryStatus;
        db.update_entry_status(entry_id, &EntryStatus::Expired, None).map_err(|e| e.to_string())?;

        Ok(format!("✓ Entry {} expired\n  content: {}", entry_id, &entry.content[..50.min(entry.content.len())]))
    }

    fn tool_supersede(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let old_id = args.get("old_id").and_then(|i| i.as_str()).ok_or("missing 'old_id'")?;
        let new_id = args.get("new_id").and_then(|i| i.as_str()).ok_or("missing 'new_id'")?;

        let db = vault.database().map_err(|e| e.to_string())?;
        let old_entry = db.get_entry(old_id).map_err(|e| e.to_string())?
            .ok_or(format!("old entry '{}' not found", old_id))?;
        let new_entry = db.get_entry(new_id).map_err(|e| e.to_string())?
            .ok_or(format!("new entry '{}' not found", new_id))?;

        use crate::storage::models::{EntryStatus, EntryLink, LinkRelation};
        db.update_entry_status(old_id, &EntryStatus::Superseded, Some(new_id)).map_err(|e| e.to_string())?;
        db.add_entry_link(new_id, &EntryLink { target_id: old_id.to_string(), relation: LinkRelation::Supersedes }).map_err(|e| e.to_string())?;

        Ok(format!("✓ {} superseded by {}\n  old: {}\n  new: {}", old_id, new_id,
            &old_entry.content[..50.min(old_entry.content.len())],
            &new_entry.content[..50.min(new_entry.content.len())]))
    }

    fn tool_recall(_args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let ctx_id = vault.current_context.as_ref()
            .ok_or("No active context.")?;

        let db = vault.database().map_err(|e| e.to_string())?;
        let entries = db.list_entries(ctx_id).map_err(|e| e.to_string())?;

        use crate::storage::models::EntryStatus;
        let now = chrono::Utc::now();
        let recent_cutoff = (now - chrono::Duration::hours(24)).format("%Y-%m-%d %H:%M").to_string();

        let mut recent = Vec::new();
        let mut superseded = Vec::new();

        for e in &entries {
            match e.status {
                EntryStatus::Active => {
                    if e.date >= recent_cutoff {
                        recent.push(e);
                    }
                }
                EntryStatus::Superseded => {
                    superseded.push(e);
                }
                _ => {}
            }
        }

        let mut output = String::new();

        if !recent.is_empty() {
            output.push_str("📍 Recent (24h):\n");
            for e in recent.iter().take(10) {
                output.push_str(&format!("  [{}] {}\n", e.id, &e.content[..50.min(e.content.len())]));
            }
        }

        if !superseded.is_empty() {
            output.push_str("\n⬜ Superseded:\n");
            for e in superseded.iter().take(5) {
                let by = e.superseded_by.as_deref().unwrap_or("?");
                output.push_str(&format!("  [{}] {} → {}\n", e.id, &e.content[..40.min(e.content.len())], by));
            }
        }

        if output.is_empty() {
            output = "Nothing to recall.".to_string();
        }

        Ok(output)
    }

    fn tool_query(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let db = vault.database().map_err(|e| e.to_string())?;

        let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
        let context_id = if all {
            None
        } else if let Some(cid) = args.get("context_id").and_then(|c| c.as_str()) {
            Some(cid.to_string())
        } else {
            Some(vault.current_context.as_ref()
                .ok_or("No active context. Provide context_id or set all=true.")?
                .clone())
        };

        let from_date = if let Some(days) = args.get("last_days").and_then(|d| d.as_i64()) {
            Some((chrono::Utc::now() - chrono::Duration::days(days))
                .format("%Y-%m-%d")
                .to_string())
        } else {
            args.get("from_date").and_then(|d| d.as_str()).map(|s| s.to_string())
        };

        let entry_types: Vec<String> = args.get("entry_types")
            .and_then(|t| serde_json::from_value(t.clone()).ok())
            .unwrap_or_default();
        let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(20) as usize;

        let filter = crate::storage::QueryFilter {
            context_id,
            all_contexts: all,
            from_date,
            to_date: args.get("to_date").and_then(|d| d.as_str()).map(|s| s.to_string()),
            entry_types,
            keyword: args.get("keyword").and_then(|k| k.as_str()).map(|s| s.to_string()),
            resolved: args.get("resolved").and_then(|r| r.as_bool()),
            limit,
        };

        let entries = db.query_entries(&filter).map_err(|e| e.to_string())?;

        let ctx_titles: std::collections::HashMap<String, String> = if all {
            db.list_contexts().map_err(|e| e.to_string())?
                .into_iter()
                .map(|c| (c.id, c.title))
                .collect()
        } else {
            std::collections::HashMap::new()
        };

        let mut lines = Vec::new();
        for e in &entries {
            let content: String = e.content.replace('\n', " ");
            let truncated: String = content.chars().take(120).collect();
            if all {
                let ctx_name = ctx_titles.get(&e.context_id).map(|s| s.as_str()).unwrap_or(&e.context_id);
                lines.push(format!(
                    "{} | {} | {} | {} | resolved:{} | {}",
                    e.id, ctx_name, e.date, e.entry_type, e.resolved, truncated
                ));
            } else {
                lines.push(format!(
                    "{} | {} | {} | resolved:{} | {}",
                    e.id, e.date, e.entry_type, e.resolved, truncated
                ));
            }
        }
        lines.push(format!("Total: {} entries", entries.len()));
        Ok(lines.join("\n"))
    }

    fn tool_resolve(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let entry_id = args.get("entry_id").and_then(|i| i.as_str()).ok_or("missing 'entry_id'")?;

        let db = vault.database().map_err(|e| e.to_string())?;
        db.get_entry(entry_id).map_err(|e| e.to_string())?
            .ok_or(format!("entry '{}' not found", entry_id))?;
        db.update_entry_resolved(entry_id, true).map_err(|e| e.to_string())?;

        Ok(format!("✓ Entry {} resolved", entry_id))
    }

    fn tool_append(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let entry_id = args.get("entry_id").and_then(|i| i.as_str()).ok_or("missing 'entry_id'")?;
        let content = args.get("content").and_then(|c| c.as_str()).ok_or("missing 'content'")?;

        let db = vault.database().map_err(|e| e.to_string())?;
        db.get_entry(entry_id).map_err(|e| e.to_string())?
            .ok_or(format!("entry '{}' not found", entry_id))?;
        db.append_entry_content(entry_id, content).map_err(|e| e.to_string())?;

        Ok(format!("✓ Appended to {}", entry_id))
    }

    fn tool_stats(_args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let db = vault.database().map_err(|e| e.to_string())?;
        let stats = db.get_stats().map_err(|e| e.to_string())?;

        let mut out = format!(
            "Besure AI — Stats\n\nTotal: {} contexts, {} entries\n\nBy Context:\n",
            stats.total_contexts, stats.total_entries
        );
        for (title, count) in &stats.by_context {
            out.push_str(&format!("  {}  {} entries\n", title, count));
        }
        out.push_str("\nBy Type:\n");
        for (t, count) in &stats.by_type {
            out.push_str(&format!("  {}  {}\n", t, count));
        }
        out.push_str("\nBy Status:\n");
        for (s, count) in &stats.by_status {
            out.push_str(&format!("  {}  {}\n", s, count));
        }
        let pct = if stats.total_entries > 0 {
            (stats.resolved_count as f64 / stats.total_entries as f64 * 100.0).round() as i64
        } else {
            0
        };
        out.push_str(&format!(
            "\nResolved: {} / {} ({}%)\n",
            stats.resolved_count, stats.total_entries, pct
        ));
        if !stats.recent_activity.is_empty() {
            out.push_str("\nRecent Activity (last 7 days):\n");
            for (date, count) in &stats.recent_activity {
                out.push_str(&format!("  {}: {} entries\n", date, count));
            }
        }
        Ok(out)
    }

    fn tool_vaults(_args: &Value) -> Result<String, String> {
        if !crate::storage::Vault::can_access_all_vaults() {
            return Err("Global vault access not enabled. Set BESURE_VAULTS_ALL=true".to_string());
        }
        let vaults = crate::storage::Vault::list_vault_dirs();
        if vaults.is_empty() {
            return Ok("No vaults found.".to_string());
        }
        let current = crate::storage::Vault::default_root();
        let mut lines = Vec::new();
        for (name, path) in &vaults {
            let marker = if path == &current { "▶" } else { " " };
            let count = crate::storage::Vault::open(Some(path.clone()))
                .ok()
                .and_then(|v| v.database().ok())
                .and_then(|db| db.count_entries().ok())
                .unwrap_or(0);
            lines.push(format!("{} {} ({} entries)", marker, name, count));
        }
        lines.push(format!("\n{} vaults total", vaults.len()));
        Ok(lines.join("\n"))
    }

    fn tool_share(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let entry_id = args.get("entry_id").and_then(|i| i.as_str()).ok_or("missing 'entry_id'")?;

        let db = vault.database().map_err(|e| e.to_string())?;
        let entry = db.get_entry(entry_id).map_err(|e| e.to_string())?
            .ok_or(format!("entry '{}' not found", entry_id))?;

        let source_name = crate::storage::Vault::default_root()
            .file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or("unknown".to_string());

        let shared_path = crate::storage::Vault::shared_root();
        if !crate::storage::Vault::exists(Some(shared_path.clone())) {
            crate::storage::Vault::init(Some(shared_path.clone()), None).map_err(|e| e.to_string())?;
        }
        let shared_vault = crate::storage::Vault::open(Some(shared_path.clone())).map_err(|e| e.to_string())?;
        let shared_db = shared_vault.database().map_err(|e| e.to_string())?;

        let mut shared_entry = entry.clone();
        shared_entry.id = format!("shared_{}", entry.id);
        shared_entry.context_id = format!("ctx_shared_from_{}", source_name);

        if shared_db.get_context(&shared_entry.context_id).map_err(|e| e.to_string())?.is_none() {
            let mut ctx = crate::storage::models::Context::from_title(&format!("Shared from {}", source_name));
            ctx.id = shared_entry.context_id.clone();
            shared_db.upsert_context(&ctx).map_err(|e| e.to_string())?;
        }
        shared_entry.tags.push(format!("shared_from:{}", source_name));
        shared_db.add_entry(&shared_entry).map_err(|e| e.to_string())?;

        Ok(format!("✓ Shared entry {} to shared vault", entry_id))
    }

    fn tool_delete_entry(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let entry_id = args.get("entry_id").and_then(|i| i.as_str()).ok_or("missing 'entry_id'")?;

        let db = vault.database().map_err(|e| e.to_string())?;
        let entry = db.get_entry(entry_id).map_err(|e| e.to_string())?
            .ok_or(format!("entry '{}' not found", entry_id))?;
        db.soft_delete_entry(entry_id).map_err(|e| e.to_string())?;

        Ok(format!("✓ Entry {} moved to trash (restore with besure_restore_entry)\n  content: {}", entry_id, &entry.content[..50.min(entry.content.len())]))
    }

    fn tool_restore_entry(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let entry_id = args.get("entry_id").and_then(|i| i.as_str()).ok_or("missing 'entry_id'")?;

        let db = vault.database().map_err(|e| e.to_string())?;
        db.get_entry(entry_id).map_err(|e| e.to_string())?
            .ok_or(format!("entry '{}' not found", entry_id))?;
        db.restore_entry(entry_id).map_err(|e| e.to_string())?;

        Ok(format!("✓ Entry {} restored from trash", entry_id))
    }

    fn tool_shared(args: &Value) -> Result<String, String> {
        let shared_path = crate::storage::Vault::shared_root();
        if !crate::storage::Vault::exists(Some(shared_path.clone())) {
            return Ok("No shared vault found.".to_string());
        }
        let vault = crate::storage::Vault::open(Some(shared_path.clone())).map_err(|e| e.to_string())?;
        let db = vault.database().map_err(|e| e.to_string())?;

        let keyword = args.get("keyword").and_then(|k| k.as_str());
        let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(20) as usize;

        let filter = crate::storage::QueryFilter {
            context_id: None, all_contexts: true,
            from_date: None, to_date: None,
            entry_types: vec![],
            keyword: keyword.map(|s| s.to_string()),
            resolved: None, limit,
        };
        let entries = db.query_entries(&filter).map_err(|e| e.to_string())?;
        if entries.is_empty() { return Ok("No shared entries.".to_string()); }

        let mut lines = vec![format!("📦 Shared Vault ({} entries)", entries.len())];
        for e in &entries {
            let content: String = e.content.replace('\n', " ");
            let source = e.tags.iter()
                .find(|t| t.starts_with("shared_from:"))
                .map(|t| t.strip_prefix("shared_from:").unwrap_or(""))
                .unwrap_or("?");
            lines.push(format!("{} | from:{} | {} | {} | {}", e.id, source, e.date, e.entry_type,
                &content[..100.min(content.len())]));
        }
        Ok(lines.join("\n"))
    }
}
