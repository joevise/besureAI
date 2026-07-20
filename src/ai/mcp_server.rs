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
                        "version": "0.5.5"
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
                "name": "besure_config_set",
                "description": "设置项目配置（仓库/服务器/密钥引用等）",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "key": {"type": "string", "description": "配置键名"},
                        "value": {"type": "string", "description": "配置值"}
                    },
                    "required": ["key", "value"]
                }
            }),
            json!({
                "name": "besure_config_get",
                "description": "读取项目配置",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "key": {"type": "string", "description": "配置键名"}
                    },
                    "required": ["key"]
                }
            }),
            json!({
                "name": "besure_config_list",
                "description": "列出当前上下文的所有配置",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
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
            "besure_link" => Self::tool_link(&args),
            "besure_expire" => Self::tool_expire(&args),
            "besure_supersede" => Self::tool_supersede(&args),
            "besure_config_set" => Self::tool_config_set(&args),
            "besure_config_get" => Self::tool_config_get(&args),
            "besure_config_list" => Self::tool_config_list(&args),
            "besure_recall" => Self::tool_recall(&args),
            "besure_query" => Self::tool_query(&args),
            "besure_resolve" => Self::tool_resolve(&args),
            "besure_append" => Self::tool_append(&args),
            "besure_stats" => Self::tool_stats(&args),
            "besure_vaults" => Self::tool_vaults(&args),
            "besure_share" => Self::tool_share(&args),
            "besure_shared" => Self::tool_shared(&args),
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

    fn tool_config_set(args: &Value) -> Result<String, String> {
        let mut vault = Self::get_vault()?;
        let key = args.get("key").and_then(|k| k.as_str()).ok_or("missing 'key'")?;
        let value = args.get("value").and_then(|v| v.as_str()).ok_or("missing 'value'")?;

        let ctx_id = vault.current_context.as_ref()
            .ok_or("No active context. Switch to a context first.")?;

        let content = format!("{}: {}", key, value);
        let entry = Entry::new(ctx_id, &content, "config");
        let db = vault.database().map_err(|e| e.to_string())?;
        db.add_entry(&entry).map_err(|e| e.to_string())?;

        Ok(format!("✓ Config set: {} = {}", key, value))
    }

    fn tool_config_get(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let key = args.get("key").and_then(|k| k.as_str()).ok_or("missing 'key'")?;
        let ctx_id = vault.current_context.as_ref()
            .ok_or("No active context.")?;

        let db = vault.database().map_err(|e| e.to_string())?;
        let entries = db.list_entries(ctx_id).map_err(|e| e.to_string())?;

        let prefix = format!("{}:", key);
        let found: Vec<_> = entries.iter()
            .filter(|e| e.entry_type == "config" && e.content.starts_with(&prefix))
            .collect();

        if found.is_empty() {
            return Err(format!("Config '{}' not found", key));
        }

        let results: Vec<String> = found.iter()
            .map(|e| e.content.strip_prefix(&prefix).unwrap_or(&e.content).trim().to_string())
            .collect();
        Ok(format!("{} = {}", key, results.join(", ")))
    }

    fn tool_config_list(args: &Value) -> Result<String, String> {
        let vault = Self::get_vault()?;
        let ctx_id = vault.current_context.as_ref()
            .ok_or("No active context.")?;

        let db = vault.database().map_err(|e| e.to_string())?;
        let entries = db.list_entries(ctx_id).map_err(|e| e.to_string())?;

        let configs: Vec<_> = entries.iter()
            .filter(|e| e.entry_type == "config")
            .collect();

        if configs.is_empty() {
            return Ok("No config entries.".to_string());
        }

        let lines: Vec<String> = configs.iter()
            .map(|e| format!("  {}", e.content))
            .collect();
        Ok(format!("Config ({}):\n{}", ctx_id, lines.join("\n")))
    }

    fn tool_recall(args: &Value) -> Result<String, String> {
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
