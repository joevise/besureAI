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
                        "version": "0.1.0"
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
                "description": "向某上下文追加进展记录",
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
                "description": "搜索所有上下文（全文匹配）",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "搜索关键词"}
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
                "description": "导出上下文为 Markdown",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string", "description": "上下文 ID"},
                        "output": {"type": "string", "description": "输出文件路径"}
                    },
                    "required": ["id"]
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
        db.add_entry(&entry).map_err(|e| e.to_string())?;
        vault.write_entry_md(&entry).map_err(|e| e.to_string())?;

        Ok(format!("✓ Added {} entry to {}", entry_type, id))
    }

    fn tool_search(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let query = args.get("query").and_then(|q| q.as_str()).ok_or("missing 'query'")?;
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
        let default_output = format!("{}.md", id);
        let output = args.get("output").and_then(|o| o.as_str())
            .unwrap_or(&default_output);

        let db = vault.database().map_err(|e| e.to_string())?;
        let ctx = db.get_context(id).map_err(|e| e.to_string())?
            .ok_or(format!("context '{}' not found", id))?;
        let entries = db.list_entries(id).map_err(|e| e.to_string())?;

        let output_path = std::path::PathBuf::from(output);
        vault.export_context(&ctx, &entries, &output_path).map_err(|e| e.to_string())?;

        Ok(format!("✓ Exported '{}' to {} ({} entries)", ctx.title, output, entries.len()))
    }
}
