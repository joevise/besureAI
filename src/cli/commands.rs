use anyhow::{bail, Context as AnyhowContext, Result};
use std::io::{self, Write};
use std::path::PathBuf;

use besure_lib::ai::{EmbeddingProvider, embedding::EmbeddingConfig, Absorber, absorb::LlmConfig, VectorStore};
use besure_lib::storage::{Vault, Context, Entry, EntryLink, EntryStatus, LinkRelation};

fn read_password(prompt: &str) -> Result<String> {
    eprint!("{}", prompt);
    io::stderr().flush()?;
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;
    Ok(password.trim().to_string())
}

fn get_vault() -> Result<Vault> {
    if !Vault::exists(None) {
        bail!("Vault not initialized. Run 'besure init' first.");
    }
    Vault::open(None)
}

fn get_unlocked_vault() -> Result<Vault> {
    let vault = get_vault()?;
    if vault.config.encryption && !vault.is_unlocked() {
        bail!("Vault is locked. Run 'besure unlock' first.");
    }
    Ok(vault)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}…", truncated)
    }
}

// === init ===
pub fn cmd_init_from_args(encrypt: bool) -> Result<()> {
    if Vault::exists(None) {
        bail!("Vault already exists at ~/.besure/. Use 'besure unlock' to continue.");
    }
    let password = if encrypt {
        let pw1 = read_password("Set master password: ")?;
        let pw2 = read_password("Confirm password: ")?;
        if pw1 != pw2 {
            bail!("Passwords do not match.");
        }
        if pw1.len() < 4 {
            bail!("Password too short (minimum 4 characters).");
        }
        Some(pw1)
    } else {
        None
    };

    let vault = Vault::init(None, password.as_deref())?;
    println!("✓ Besure vault initialized at {}", vault.root.display());
    if encrypt {
        println!("🔒 Encryption enabled (AES-256-GCM + Argon2id)");
    } else {
        println!("⚠️  No encryption (data stored in plaintext)");
    }
    println!("\nGet started:");
    println!("  besure create \"My First Project\"");
    Ok(())
}

// === create ===
pub fn cmd_create_from_args(title: &str, tags: &[String], summary: Option<&str>) -> Result<()> {
    let mut vault = get_unlocked_vault()?;

    let mut ctx = Context::from_title(title);
    ctx.tags = tags.to_vec();
    ctx.summary = summary.unwrap_or("").to_string();

    let db = vault.database()?;
    db.upsert_context(&ctx)?;
    vault.write_context_md(&ctx)?;
    vault.set_current(&ctx.id)?;

    let entry = Entry::new(&ctx.id, &format!("上下文初始化: {}", ctx.title), "init");
    let db = vault.database()?;
    db.add_entry(&entry)?;
    vault.write_entry_md(&entry)?;

    println!("✓ Created context: {} ({})", ctx.title, ctx.id);
    println!("✓ Switched to this context (current)");
    Ok(())
}

// === switch ===
pub fn cmd_switch_from_args(query: &str) -> Result<()> {
    let mut vault = get_unlocked_vault()?;
    let db = vault.database()?;

    // Exact match
    if let Some(ctx) = db.get_context(query)? {
        vault.set_current(&ctx.id)?;
        println!("✓ Switched to: {} ({})", ctx.title, ctx.id);
        return Ok(());
    }

    // Fuzzy match
    let found = db.find_contexts_fuzzy(query)?;
    match found.len() {
        0 => bail!("No context found matching '{}'", query),
        1 => {
            let ctx = &found[0];
            vault.set_current(&ctx.id)?;
            println!("✓ Switched to: {} ({})", ctx.title, ctx.id);
        }
        _ => {
            println!("Multiple contexts found:");
            for (i, ctx) in found.iter().enumerate() {
                println!("  [{}] {} ({}) [{}]", i + 1, ctx.title, ctx.id, ctx.status);
            }
            print!("\nEnter number to switch: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let num: usize = input.trim().parse().context("invalid number")?;
            if num < 1 || num > found.len() {
                bail!("invalid selection");
            }
            let ctx = &found[num - 1];
            vault.set_current(&ctx.id)?;
            println!("✓ Switched to: {} ({})", ctx.title, ctx.id);
        }
    }
    Ok(())
}

// === add ===
pub fn cmd_add_from_args(content: Option<&str>, from_file: Option<&str>, entry_type: &str) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let context_id = vault
        .current_context
        .as_ref()
        .context("No active context. Run 'besure create' or 'besure switch' first.")?;

    let final_content = if let Some(path) = from_file {
        std::fs::read_to_string(path)
            .with_context(|| format!("failed to read file: {}", path))?
    } else if let Some(c) = content {
        c.to_string()
    } else {
        bail!("No content provided. Use positional text or --from-file <path>")
    };

    let entry = Entry::new(context_id, &final_content, entry_type);

    let db = vault.database()?;
    db.add_entry(&entry)?;
    vault.write_entry_md(&entry)?;

    println!("✓ Added {} entry to {}", entry_type, context_id);
    Ok(())
}

// === list ===
pub fn cmd_list() -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;
    let contexts = db.list_contexts()?;

    if contexts.is_empty() {
        println!("No contexts yet. Run 'besure create \"Project Name\"' to create one.");
        return Ok(());
    }

    let current = &vault.current_context;

    println!(
        "{:<3}{:<28} {:<14} {:<10} {:<10}",
        "", "TITLE", "ID", "STATUS", "TAGS"
    );
    println!("{}", "-".repeat(80));
    for ctx in &contexts {
        let marker = if current.as_deref() == Some(&ctx.id) {
            "▶ "
        } else {
            "  "
        };
        let tags = if ctx.tags.is_empty() {
            "-".to_string()
        } else {
            ctx.tags.join(",")
        };
        println!(
            "{}{:<28} {:<14} {:<10} {:<10}",
            marker,
            truncate(&ctx.title, 28),
            truncate(&ctx.id, 14),
            ctx.status,
            truncate(&tags, 10)
        );
    }
    println!("\n{} contexts total", contexts.len());
    drop(contexts);
    Ok(())
}

// === log ===
pub fn cmd_log_from_args(context: Option<&str>) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    let context_id = if let Some(id) = context {
        let found = db.find_contexts_fuzzy(id)?;
        if found.len() == 1 {
            found[0].id.clone()
        } else if found.is_empty() {
            bail!("No context found matching '{}'", id);
        } else {
            found[0].id.clone()
        }
    } else {
        vault
            .current_context
            .as_ref()
            .context("No active context")?
            .clone()
    };

    let ctx = db.get_context(&context_id)?.context("context not found")?;
    let entries = db.list_entries(&context_id)?;

    println!("═══ {} ({}) ═══", ctx.title, ctx.id);
    println!(
        "Status: {} | Created: {} | Updated: {}\n",
        ctx.status, ctx.created, ctx.updated
    );

    if entries.is_empty() {
        println!("No entries yet.");
    } else {
        for (i, entry) in entries.iter().enumerate() {
            let status_marker = match entry.status {
                EntryStatus::Active => "",
                EntryStatus::Superseded => " [superseded]",
                EntryStatus::Expired => " [expired]",
                EntryStatus::Archived => " [archived]",
            };
            println!(
                "┌─ [{}] {} ({}){}",
                entries.len() - i,
                entry.date,
                entry.entry_type,
                status_marker
            );
            println!("│ {}", entry.content);
            if !entry.links.is_empty() {
                let links_str: Vec<String> = entry.links.iter()
                    .map(|l| format!("{}({})", l.relation, l.target_id))
                    .collect();
                println!("│ 🔗 {}", links_str.join(", "));
            }
            if let Some(ref vu) = entry.valid_until {
                println!("│ ⏰ expires: {}", vu);
            }
            println!("└─\n");
        }
    }
    Ok(())
}

// === search ===
pub fn cmd_search_from_args(query: &str, semantic: bool) -> Result<()> {
    if semantic { return do_semantic_search(query); }
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    let results = db.search(query)?;

    if results.is_empty() {
        println!("No results for '{}'.", query);
        return Ok(());
    }

    println!("Search results for \"{}\":\n", query);
    let mut current_ctx = String::new();
    for (ctx, entry) in &results {
        if ctx.id != current_ctx {
            current_ctx = ctx.id.clone();
            println!("─── {} ({}) ───", ctx.title, ctx.id);
        }
        println!(
            "  [{}] {} | {}",
            entry.date,
            entry.entry_type,
            truncate(entry.content.trim(), 60)
        );
    }
    println!("\n{} results found.", results.len());
    Ok(())
}

// === export ===
pub fn cmd_export_from_args(context: &str, output: Option<&str>) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    let found = db.find_contexts_fuzzy(context)?;
    let ctx = if found.len() == 1 {
        found[0].clone()
    } else if found.is_empty() {
        bail!("No context found matching '{}'", context);
    } else {
        println!("Multiple contexts found:");
        for (i, c) in found.iter().enumerate() {
            println!("  [{}] {} ({})", i + 1, c.title, c.id);
        }
        print!("Enter number: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let num: usize = input.trim().parse()?;
        found[num - 1].clone()
    };

    let entries = db.list_entries(&ctx.id)?;

    let output = output
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}.md", ctx.id));

    let output_path = PathBuf::from(&output);
    vault.export_context(&ctx, &entries, &output_path)?;

    println!(
        "✓ Exported '{}' to {} ({} entries)",
        ctx.title,
        output,
        entries.len()
    );
    Ok(())
}

// === unlock ===
pub fn cmd_unlock() -> Result<()> {
    let mut vault = get_vault()?;

    if !vault.config.encryption {
        println!("Vault has no encryption. Nothing to unlock.");
        return Ok(());
    }

    if vault.is_unlocked() {
        println!("Vault is already unlocked.");
        return Ok(());
    }

    let password = read_password("Master password: ")?;
    vault.unlock(&password)?;
    println!("🔓 Vault unlocked.");
    Ok(())
}

// === lock ===
pub fn cmd_lock() -> Result<()> {
    let mut vault = get_vault()?;

    if !vault.config.encryption {
        println!("Vault has no encryption. Nothing to lock.");
        return Ok(());
    }

    vault.lock()?;
    println!("🔒 Vault locked.");
    Ok(())
}

// === status ===
pub fn cmd_status() -> Result<()> {
    let vault = get_unlocked_vault()?;
    let (ctx_count, entry_count, current) = vault.status_summary()?;
    let current: Option<String> = current;

    let lock_status = if vault.config.encryption {
        if vault.is_unlocked() {
            "🔓 Unlocked"
        } else {
            "🔒 Locked"
        }
    } else {
        "⚪ No encryption"
    };

    println!("Besure AI — 貔貅记忆\n");
    println!("Vault:    {}", vault.root.display());
    println!("Status:   {}", lock_status);
    println!("Contexts: {}", ctx_count);
    println!("Entries:  {}", entry_count);

    if let Some(ref ctx_id) = current {
        let db = vault.database()?;
        if let Some(ctx) = db.get_context(&ctx_id)? {
            println!("\nCurrent context:");
            println!("  ▶ {} ({})", ctx.title, ctx.id);
            println!("    Status: {} | Updated: {}", ctx.status, ctx.updated);
            if !ctx.summary.is_empty() {
                println!("    Summary: {}", truncate(&ctx.summary, 60));
            }
        }
    } else {
        println!("\nNo active context. Run 'besure create' or 'besure switch'.");
    }

    Ok(())
}

// === absorb ===
pub fn cmd_absorb_from_args(from: Option<&str>, auto: bool) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let config = load_config()?;
    let absorber = Absorber::new(config.llm);

    let entries = if let Some(path) = from {
        absorber.absorb_file(std::path::Path::new(path))?
    } else {
        absorber.absorb_stdin()?
    };

    if entries.is_empty() {
        println!("未提取到进展记录。");
        return Ok(());
    }

    println!("提取到 {} 条进展：\n", entries.len());
    for (i, entry) in entries.iter().enumerate() {
        println!("  [{}] ({}) {}", i + 1, entry.entry_type, entry.content);
    }

    if auto {
        let context_id = vault.current_context.as_ref().context("No active context")?;
        let db = vault.database()?;
        for entry in &entries {
            let e = Entry::new(context_id, &entry.content, &entry.entry_type);
            db.add_entry(&e)?;
            vault.write_entry_md(&e)?;
        }
        println!("\n✓ 已自动添加 {} 条到 {}", entries.len(), context_id);
    } else {
        println!("\n使用 --auto 自动添加到当前上下文");
    }
    Ok(())
}

// === config set (app config: embedding/llm etc.) ===
pub fn cmd_config_set(key: &str, value: &str) -> Result<()> {
    let mut config = load_config()?;
    match key {
        "embedding.provider" => config.embedding.provider = value.to_string(),
        "embedding.api_url" => config.embedding.api_url = value.to_string(),
        "embedding.api_key" => config.embedding.api_key = value.to_string(),
        "embedding.model" => config.embedding.model = value.to_string(),
        "llm.provider" => config.llm.provider = value.to_string(),
        "llm.api_url" => config.llm.api_url = value.to_string(),
        "llm.api_key" => config.llm.api_key = value.to_string(),
        "llm.model" => config.llm.model = value.to_string(),
        "auto_lock_minutes" => config.auto_lock_minutes = value.parse().context("invalid number")?,
        _ => bail!("unknown config key: {}", key),
    }
    save_config(&config)?;
    println!("✓ {} = {}", key, value);
    Ok(())
}

// === semantic search ===
fn do_semantic_search(query: &str) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let config = load_config()?;
    let provider = EmbeddingProvider::new(config.embedding);
    let query_vec = provider.embed(query)?;

    let vec_db_path = vault.root.join("vectors.db");
    if !vec_db_path.exists() {
        bail!("No vectors indexed yet. Run 'besure index' to build the index.");
    }

    let store = VectorStore::open(&vec_db_path)?;
    let results = store.search(&query_vec, 10)?;

    if results.is_empty() {
        println!("No results for '{}'.", query);
        return Ok(());
    }

    println!("Semantic search results for \"{}\":\n", query);
    for (i, r) in results.iter().enumerate() {
        println!("  [{}] {:.2} | {} | {}", i + 1, r.score, r.context_id, truncate(&r.chunk_text, 50));
    }
    Ok(())
}

// === NEW: link command ===
/// `besure link <entry_id> --to <target_id> [--as <relation>]`
pub fn cmd_link(entry_id: &str, target_id: &str, as_relation: Option<&str>) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    // Verify entry exists
    let entry = db.get_entry(entry_id)?.context("entry not found")?;

    // Parse relation (default: related_to)
    let relation: LinkRelation = match as_relation {
        Some(r) => r.parse().map_err(|e: String| anyhow::anyhow!("{}", e))?,
        None => LinkRelation::RelatedTo,
    };

    let link = EntryLink {
        target_id: target_id.to_string(),
        relation: relation.clone(),
    };

    db.add_entry_link(entry_id, &link)?;

    println!("✓ Linked {} → {} ({})", entry_id, target_id, relation);
    Ok(())
}

// === NEW: expire command ===
/// `besure expire <entry_id>`
pub fn cmd_expire(entry_id: &str) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    // Verify entry exists
    let entry = db.get_entry(entry_id)?.context("entry not found")?;

    db.update_entry_status(entry_id, &EntryStatus::Expired, None)?;

    println!("✓ Entry {} expired", entry_id);
    println!("  content: {}", truncate(&entry.content, 60));
    Ok(())
}

// === NEW: supersede command ===
/// `besure supersede <old_id> <new_id>`
pub fn cmd_supersede(old_id: &str, new_id: &str) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    // Verify both entries exist
    let old_entry = db.get_entry(old_id)?.context("old entry not found")?;
    let new_entry = db.get_entry(new_id)?.context("new entry not found")?;

    // Set old entry to Superseded with pointer to new
    db.update_entry_status(old_id, &EntryStatus::Superseded, Some(new_id))?;

    // Add Supersedes link from new entry to old
    let link = EntryLink {
        target_id: old_id.to_string(),
        relation: LinkRelation::Supersedes,
    };
    db.add_entry_link(new_id, &link)?;

    println!("✓ {} superseded by {}", old_id, new_id);
    println!("  old: {}", truncate(&old_entry.content, 50));
    println!("  new: {}", truncate(&new_entry.content, 50));
    Ok(())
}

// === NEW: config entry commands ===
/// `besure config set <key> <value>` — stores as entry with type "config"
pub fn cmd_config_set_entry(key: &str, value: &str) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let context_id = vault
        .current_context
        .as_ref()
        .context("No active context. Run 'besure switch' first.")?;

    let content = format!("{}: {}", key, value);
    let entry = Entry::new(context_id, &content, "config");

    let db = vault.database()?;
    db.add_entry(&entry)?;
    vault.write_entry_md(&entry)?;

    println!("✓ Config set: {} = {}", key, value);
    Ok(())
}

/// `besure config get <key>` — searches config entries by content prefix
pub fn cmd_config_get(key: &str) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;
    let context_id = vault
        .current_context
        .as_ref()
        .context("No active context")?;

    let entries = db.list_entries(context_id)?;
    let prefix = format!("{}:", key);

    let found: Vec<_> = entries
        .iter()
        .filter(|e| e.entry_type == "config" && e.content.starts_with(&prefix))
        .collect();

    if found.is_empty() {
        println!("No config found for key '{}'", key);
        return Ok(());
    }

    for entry in found {
        let value = entry.content.strip_prefix(&prefix).unwrap_or("").trim();
        println!("{} = {}", key, value);
    }
    Ok(())
}

/// `besure config list` — list all config entries in current context
pub fn cmd_config_list() -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;
    let context_id = vault
        .current_context
        .as_ref()
        .context("No active context")?;

    let entries = db.list_entries(context_id)?;
    let configs: Vec<_> = entries.iter().filter(|e| e.entry_type == "config").collect();

    if configs.is_empty() {
        println!("No config entries in current context.");
        return Ok(());
    }

    println!("Config ({}):", context_id);
    for entry in configs {
        let line = entry.content.clone();
        println!("  {}", line);
    }
    Ok(())
}

// === NEW: recall command ===
/// `besure recall` — returns entries that need attention
pub fn cmd_recall() -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    let context_id = vault
        .current_context
        .as_ref()
        .context("No active context")?;

    let now = chrono::Utc::now();
    let now_str = now.format("%Y-%m-%d").to_string();
    let seven_days_later = (now + chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let twenty_four_h_ago = (now - chrono::Duration::hours(24)).format("%Y-%m-%d %H:%M").to_string();

    let entries = db.list_entries(context_id)?;

    let mut expiring_soon: Vec<&Entry> = Vec::new();
    let mut overdue: Vec<&Entry> = Vec::new();
    let mut recent: Vec<&Entry> = Vec::new();
    let mut superseded: Vec<&Entry> = Vec::new();

    for e in &entries {
        match e.status {
            EntryStatus::Active => {
                if let Some(ref vu) = e.valid_until {
                    if vu.as_str() < now_str.as_str() {
                        overdue.push(e);
                    } else if vu.as_str() <= seven_days_later.as_str() {
                        expiring_soon.push(e);
                    }
                }
                if e.date.as_str() >= twenty_four_h_ago.as_str() {
                    recent.push(e);
                }
            }
            EntryStatus::Superseded => {
                superseded.push(e);
            }
            _ => {}
        }
    }

    if expiring_soon.is_empty() && overdue.is_empty() && recent.is_empty() && superseded.is_empty() {
        println!("Nothing to recall. All quiet. 🧘");
        return Ok(());
    }

    if !expiring_soon.is_empty() {
        println!("⚠️  Expiring Soon:");
        for e in &expiring_soon {
            println!("  [{}] {} (expires {})",
                e.id, truncate(e.content.trim(), 50), e.valid_until.as_ref().unwrap());
        }
        println!();
    }

    if !overdue.is_empty() {
        println!("🔴 Overdue:");
        for e in &overdue {
            println!("  [{}] {} (expired {})",
                e.id, truncate(e.content.trim(), 50), e.valid_until.as_ref().unwrap());
        }
        println!();
    }

    if !recent.is_empty() {
        println!("📍 Recent (24h):");
        for e in &recent {
            println!("  [{}] {}", e.id, truncate(e.content.trim(), 50));
        }
        println!();
    }

    if !superseded.is_empty() {
        println!("⬜ Superseded:");
        for e in &superseded {
            let by = e.superseded_by.as_deref().unwrap_or("?");
            println!("  [{}] {} → {}", e.id, truncate(e.content.trim(), 40), by);
        }
    }

    Ok(())
}

// === NEW: query command ===
/// `besure query` — unified query with time/type/keyword/resolved filters
pub struct QueryArgs {
    pub last: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub entry_types: Vec<String>,
    pub all: bool,
    pub context: Option<String>,
    pub keyword: Option<String>,
    pub unresolved: bool,
    pub resolved: bool,
    pub limit: usize,
}

pub fn cmd_query(args: &QueryArgs) -> Result<()> {
    // Check --all-vaults first
    if std::env::var("BESURE_QUERY_ALL_VAULTS").is_ok() {
        return cmd_query_all_vaults(args);
    }

    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    // Resolve context
    let (context_id, all_contexts) = if args.all {
        (None, true)
    } else if let Some(ref q) = args.context {
        let found = db.find_contexts_fuzzy(q)?;
        if found.is_empty() {
            bail!("No context found matching '{}'", q);
        }
        (Some(found[0].id.clone()), false)
    } else {
        let cid = vault
            .current_context
            .as_ref()
            .context("No active context. Run 'besure switch' first or use --all.")?
            .clone();
        (Some(cid), false)
    };

    // Parse --last Nd → from_date
    let from_date = if let Some(ref last) = args.last {
        let days: i64 = last
            .trim_end_matches('d')
            .parse()
            .with_context(|| format!("invalid --last value: '{}' (expected e.g. 7d)", last))?;
        Some((chrono::Utc::now() - chrono::Duration::days(days))
            .format("%Y-%m-%d")
            .to_string())
    } else {
        args.from.clone()
    };

    let resolved = if args.resolved {
        Some(true)
    } else if args.unresolved {
        Some(false)
    } else {
        None
    };

    let filter = besure_lib::storage::QueryFilter {
        context_id,
        all_contexts,
        from_date,
        to_date: args.to.clone(),
        entry_types: args.entry_types.clone(),
        keyword: args.keyword.clone(),
        resolved,
        limit: args.limit,
    };

    let entries = db.query_entries(&filter)?;

    // Context id → title map (for --all display)
    let ctx_titles: std::collections::HashMap<String, String> = if all_contexts {
        db.list_contexts()?
            .into_iter()
            .map(|c| (c.id, c.title))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    for e in &entries {
        let content: String = e.content.replace('\n', " ");
        if all_contexts {
            let ctx_name = ctx_titles.get(&e.context_id).map(|s| s.as_str()).unwrap_or(&e.context_id);
            println!(
                "{} | {} | {} | {} | resolved:{} | {}",
                e.id, ctx_name, e.date, e.entry_type, e.resolved,
                truncate(&content, 120)
            );
        } else {
            println!(
                "{} | {} | {} | resolved:{} | {}",
                e.id, e.date, e.entry_type, e.resolved,
                truncate(&content, 120)
            );
        }
    }
    println!("Total: {} entries", entries.len());
    Ok(())
}

// === NEW: resolve command ===
/// `besure resolve <entry_id>` — mark entry as resolved
pub fn cmd_resolve(entry_id: &str) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    db.get_entry(entry_id)?.context("entry not found")?;
    db.update_entry_resolved(entry_id, true)?;

    println!("✓ Entry {} resolved", entry_id);
    Ok(())
}

// === NEW: append command ===
/// `besure append <entry_id> [content]` — append content to an existing entry
pub fn cmd_append(entry_id: &str, content: Option<&str>, from_file: Option<&str>) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    let final_content = if let Some(path) = from_file {
        std::fs::read_to_string(path)
            .with_context(|| format!("failed to read file: {}", path))?
    } else if let Some(c) = content {
        c.to_string()
    } else {
        bail!("No content provided. Use positional text or --from-file <path>")
    };

    db.get_entry(entry_id)?.context("entry not found")?;
    db.append_entry_content(entry_id, &final_content)?;

    println!("✓ Appended to {}", entry_id);
    Ok(())
}

// === NEW: stats command ===
/// `besure stats` — global statistics overview
pub fn cmd_stats() -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;
    let stats = db.get_stats()?;

    println!("Besure AI — Stats\n");
    println!(
        "Total: {} contexts, {} entries\n",
        stats.total_contexts, stats.total_entries
    );

    println!("By Context:");
    for (title, count) in &stats.by_context {
        println!("  {:<28} {} entries", truncate(title, 28), count);
    }

    println!("\nBy Type:");
    for (t, count) in &stats.by_type {
        println!("  {:<12} {}", t, count);
    }

    println!("\nBy Status:");
    for (s, count) in &stats.by_status {
        println!("  {:<12} {}", s, count);
    }

    let pct = if stats.total_entries > 0 {
        (stats.resolved_count as f64 / stats.total_entries as f64 * 100.0).round() as i64
    } else {
        0
    };
    println!(
        "\nResolved: {} / {} ({}%)",
        stats.resolved_count, stats.total_entries, pct
    );

    if !stats.recent_activity.is_empty() {
        println!("\nRecent Activity (last 7 days):");
        for (date, count) in &stats.recent_activity {
            println!("  {}: {} entries", date, count);
        }
    }

    Ok(())
}

// === V0.5: Multi-vault commands ===

/// `besure vaults` — list all vaults
pub fn cmd_vaults() -> Result<()> {
    if !Vault::can_access_all_vaults() {
        bail!("Global vault access not enabled. Set BESURE_VAULTS_ALL=true to use this command.");
    }

    let vaults = Vault::list_vault_dirs();
    if vaults.is_empty() {
        println!("No vaults found under {}", Vault::vault_parent().display());
        return Ok(());
    }

    let current_root = Vault::default_root();
    println!("{:<3}{:<20} {:<30} {}", "", "VAULT", "PATH", "ENTRIES");
    println!("{}", "-".repeat(80));
    for (name, path) in &vaults {
        let marker = if path == &current_root { "▶ " } else { "  " };
        let entry_count = match Vault::open(Some(path.clone())) {
            Ok(v) => v.database().ok().and_then(|db| db.count_entries().ok()).unwrap_or(0),
            Err(_) => -1,
        };
        println!("{}{:<20} {:<30} {} entries", marker, name, truncate(&path.display().to_string(), 30), entry_count);
    }
    println!("\n{} vaults total", vaults.len());
    Ok(())
}

/// `besure share <entry_id>` — push entry to shared vault
pub fn cmd_share(entry_id: &str) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    let entry = db.get_entry(entry_id)?.context("entry not found")?;
    let source_vault_name = Vault::default_root()
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Ensure shared vault exists
    let shared_path = Vault::shared_root();
    if !Vault::exists(Some(shared_path.clone())) {
        Vault::init(Some(shared_path.clone()), None)?;
    }

    let shared_vault = Vault::open(Some(shared_path.clone()))?;
    let shared_db = shared_vault.database()?;

    // Add entry to shared vault with source annotation
    let mut shared_entry = entry.clone();
    shared_entry.id = format!("shared_{}", entry.id);
    shared_entry.context_id = format!("ctx_shared_from_{}", source_vault_name);

    // Ensure context exists in shared vault
    if shared_db.get_context(&shared_entry.context_id)?.is_none() {
        let ctx = besure_lib::storage::Context::from_title(&format!("Shared from {}", source_vault_name));
        // Override the auto-generated id
        let mut ctx = ctx;
        ctx.id = shared_entry.context_id.clone();
        shared_db.upsert_context(&ctx)?;
    }

    // Add source tag
    shared_entry.tags.push(format!("shared_from:{}", source_vault_name));

    shared_db.add_entry(&shared_entry)?;

    println!("✓ Shared entry {} to shared vault", entry_id);
    println!("  shared_id: {}", shared_entry.id);
    Ok(())
}

/// `besure share-context <context_id>` — push entire context to shared vault
pub fn cmd_share_context(context_id: &str) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let db = vault.database()?;

    let source_vault_name = Vault::default_root()
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let ctx = db.get_context(context_id)?.context("context not found")?;
    let entries = db.list_entries(context_id)?;

    let shared_path = Vault::shared_root();
    if !Vault::exists(Some(shared_path.clone())) {
        Vault::init(Some(shared_path.clone()), None)?;
    }

    let shared_vault = Vault::open(Some(shared_path.clone()))?;
    let shared_db = shared_vault.database()?;

    // Copy context with modified id
    let mut shared_ctx = ctx.clone();
    shared_ctx.id = format!("ctx_shared_{}_{}", source_vault_name, context_id);
    shared_db.upsert_context(&shared_ctx)?;

    let entry_count = entries.len();
    for mut entry in entries {
        entry.id = format!("shared_{}", entry.id);
        entry.context_id = shared_ctx.id.clone();
        entry.tags.push(format!("shared_from:{}", source_vault_name));
        shared_db.add_entry(&entry)?;
    }

    println!("✓ Shared context {} ({} entries) to shared vault", context_id, entry_count);
    Ok(())
}

/// `besure shared` — view shared vault contents
pub fn cmd_shared(keyword: Option<&str>, entry_types: &[String], limit: usize) -> Result<()> {
    let shared_path = Vault::shared_root();
    if !Vault::exists(Some(shared_path.clone())) {
        println!("No shared vault found at {}", shared_path.display());
        return Ok(());
    }

    let vault = Vault::open(Some(shared_path.clone()))?;
    let db = vault.database()?;

    let filter = besure_lib::storage::QueryFilter {
        context_id: None,
        all_contexts: true,
        from_date: None,
        to_date: None,
        entry_types: entry_types.to_vec(),
        keyword: keyword.map(|s| s.to_string()),
        resolved: None,
        limit,
    };

    let entries = db.query_entries(&filter)?;

    if entries.is_empty() {
        println!("No shared entries.");
        return Ok(());
    }

    println!("📦 Shared Vault ({} entries)\n", entries.len());
    for e in &entries {
        let content: String = e.content.replace('\n', " ");
        let source = e.tags.iter()
            .find(|t| t.starts_with("shared_from:"))
            .map(|t| t.strip_prefix("shared_from:").unwrap_or(""))
            .unwrap_or("?");
        println!("{} | from:{} | {} | {} | resolved:{} | {}",
            e.id, source, e.date, e.entry_type, e.resolved, truncate(&content, 100));
    }
    Ok(())
}

/// Multi-vault query: query across all vaults
pub fn cmd_query_all_vaults(args: &QueryArgs) -> Result<()> {
    if !Vault::can_access_all_vaults() {
        bail!("Global vault access not enabled. Set BESURE_VAULTS_ALL=true to use --all-vaults.");
    }

    let vaults = Vault::list_vault_dirs();
    if vaults.is_empty() {
        println!("No vaults found.");
        return Ok(());
    }

    let mut total = 0;
    for (name, path) in &vaults {
        // Skip shared vault in all-vaults query (it's separate)
        if name == "shared" { continue; }

        match Vault::open(Some(path.clone())) {
            Ok(v) => {
                if let Ok(db) = v.database() {
                    // Resolve context filter
                    let context_id = if let Some(ref q) = args.context {
                        db.find_contexts_fuzzy(q)?.first().map(|c| c.id.clone())
                    } else {
                        v.current_context.clone()
                    };

                    let from_date = if let Some(ref last) = args.last {
                        let days: i64 = last.trim_end_matches('d').parse()
                            .with_context(|| format!("invalid --last: '{}'", last))?;
                        Some((chrono::Utc::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string())
                    } else { args.from.clone() };

                    let resolved = if args.resolved { Some(true) } else if args.unresolved { Some(false) } else { None };

                    let filter = besure_lib::storage::QueryFilter {
                        context_id,
                        all_contexts: args.all,
                        from_date,
                        to_date: args.to.clone(),
                        entry_types: args.entry_types.clone(),
                        keyword: args.keyword.clone(),
                        resolved,
                        limit: args.limit,
                    };

                    let entries = db.query_entries(&filter)?;
                    if !entries.is_empty() {
                        for e in &entries {
                            let content: String = e.content.replace('\n', " ");
                            println!("{} | vault:{} | {} | {} | resolved:{} | {}",
                                e.id, name, e.date, e.entry_type, e.resolved, truncate(&content, 100));
                        }
                        total += entries.len();
                    }
                }
            }
            Err(_) => continue,
        }
    }
    println!("Total: {} entries across all vaults", total);
    Ok(())
}

// === config helpers ===

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AppConfig {
    #[serde(default)]
    embedding: EmbeddingConfig,
    #[serde(default)]
    llm: LlmConfig,
    #[serde(default = "default_auto_lock")]
    auto_lock_minutes: u32,
}

fn default_auto_lock() -> u32 { 5 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            embedding: EmbeddingConfig::default(),
            llm: LlmConfig::default(),
            auto_lock_minutes: 5,
        }
    }
}

fn load_config() -> Result<AppConfig> {
    let path = Vault::default_root().join("appconfig.json");
    if path.exists() {
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json).unwrap_or_default())
    } else {
        Ok(AppConfig::default())
    }
}

fn save_config(config: &AppConfig) -> Result<()> {
    let path = Vault::default_root().join("appconfig.json");
    std::fs::create_dir_all(path.parent().unwrap())?;
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, json)?;
    Ok(())
}

// === V0.5.1: Service management ===

/// `besure service install` — 安装进程守护（三平台）
pub fn cmd_service_install() -> Result<()> {
    let bin_path = std::env::current_exe()
        .context("failed to get current exe path")?;
    let bin_path = bin_path.to_string_lossy().to_string();
    let bin_dir = std::path::Path::new(&bin_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/usr/local/bin".to_string());

    #[cfg(target_os = "linux")]
    {
        let service_dir = dirs::home_dir()
            .context("no home dir")?
            .join(".config/systemd/user");
        std::fs::create_dir_all(&service_dir)?;

        let service_content = format!(r#"[Unit]
Description=Besure AI Context Dashboard
After=network.target

[Service]
Type=simple
Environment=PATH={bin_dir}:/usr/local/bin:/usr/bin:/bin
ExecStart={bin_path} serve --port 7788
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
"#);

        let service_file = service_dir.join("besure-dashboard.service");
        std::fs::write(&service_file, &service_content)?;

        let _ = std::process::Command::new("loginctl")
            .args(["enable-linger", &std::env::var("USER").unwrap_or_default()])
            .output();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"]).output();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "enable", "besure-dashboard.service"]).output();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "restart", "besure-dashboard.service"]).output();

        println!("✓ systemd service installed and started");
        println!("  Dashboard: http://localhost:7788");
        println!("  Manage: systemctl --user {{start|stop|status}} besure-dashboard");
    }

    #[cfg(target_os = "macos")]
    {
        let plist_dir = dirs::home_dir()
            .context("no home dir")?
            .join("Library/LaunchAgents");
        std::fs::create_dir_all(&plist_dir)?;

        let plist_content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.besure.context</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin_path}</string>
        <string>serve</string>
        <string>--port</string>
        <string>7788</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardErrorPath</key>
    <string>/tmp/besure-dashboard.err</string>
    <key>StandardOutPath</key>
    <string>/tmp/besure-dashboard.out</string>
</dict>
</plist>
"#);

        let plist_path = plist_dir.join("com.besure.context.plist");
        std::fs::write(&plist_path, &plist_content)?;

        let _ = std::process::Command::new("launchctl")
            .args(["unload", &plist_path.to_string_lossy()]).output();
        let _ = std::process::Command::new("launchctl")
            .args(["load", &plist_path.to_string_lossy()]).output();

        println!("✓ launchd service installed and started");
        println!("  Dashboard: http://localhost:7788");
        println!("  Manage: launchctl {{load|unload}} ~/Library/LaunchAgents/com.besure.context.plist");
    }

    #[cfg(target_os = "windows")]
    {
        let startup_dir = dirs::data_dir()
            .context("no data dir")?
            .join("Microsoft/Windows/Start Menu/Programs/Startup");
        std::fs::create_dir_all(&startup_dir)?;

        let vbs_content = format!(
            r#"Set WshShell = CreateObject("WScript.Shell")
WshShell.Run "{} serve --port 7788", 0, False
"#,
            bin_path.replace('/', "\\\\")
        );

        let vbs_path = startup_dir.join("besure-dashboard.vbs");
        std::fs::write(&vbs_path, &vbs_content)?;

        println!("✓ Windows startup script installed");
        println!("  Location: {}", vbs_path.display());
        println!("  Dashboard: http://localhost:7788");
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        bail!("Service installation not supported on this platform.");
    }

    Ok(())
}

/// `besure service uninstall`
pub fn cmd_service_uninstall() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "stop", "besure-dashboard.service"]).output();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "besure-dashboard.service"]).output();
        let service_file = dirs::home_dir()
            .context("no home dir")?
            .join(".config/systemd/user/besure-dashboard.service");
        if service_file.exists() { std::fs::remove_file(&service_file)?; }
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"]).output();
        println!("✓ systemd service uninstalled");
    }

    #[cfg(target_os = "macos")]
    {
        let plist_path = dirs::home_dir()
            .context("no home dir")?
            .join("Library/LaunchAgents/com.besure.context.plist");
        if plist_path.exists() {
            let _ = std::process::Command::new("launchctl")
                .args(["unload", &plist_path.to_string_lossy()]).output();
            std::fs::remove_file(&plist_path)?;
        }
        println!("✓ launchd service uninstalled");
    }

    #[cfg(target_os = "windows")]
    {
        let startup_dir = dirs::data_dir()
            .context("no data dir")?
            .join("Microsoft/Windows/Start Menu/Programs/Startup");
        let vbs_path = startup_dir.join("besure-dashboard.vbs");
        if vbs_path.exists() { std::fs::remove_file(&vbs_path)?; }
        println!("✓ Windows startup script removed");
    }

    Ok(())
}

/// `besure service status`
pub fn cmd_service_status() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(o) = std::process::Command::new("systemctl")
            .args(["--user", "status", "besure-dashboard.service", "--no-pager"]).output()
        {
            println!("{}", String::from_utf8_lossy(&o.stdout));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(o) = std::process::Command::new("launchctl")
            .args(["list", "com.besure.context"]).output()
        {
            println!("{}", String::from_utf8_lossy(&o.stdout));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(o) = std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq besure.exe"]).output()
        {
            println!("{}", String::from_utf8_lossy(&o.stdout));
        }
    }

    // 通用健康检查
    if let Ok(resp) = std::process::Command::new("curl")
        .args(["-s", "http://localhost:7788/api/health"]).output()
    {
        let health = String::from_utf8_lossy(&resp.stdout);
        if health.contains("ok") {
            println!("\n✓ Dashboard is running on http://localhost:7788");
        } else {
            println!("\n✗ Dashboard is not responding");
        }
    }

    Ok(())
}

// === V0.56: besure setup ===

/// Agent config files to detect (platform-agnostic)
const AGENT_CONFIG_FILES: &[&str] = &[
    "AGENTS.md",           // OpenClaw / Codex / Hermes fallback / WorkBuddy
    ".hermes.md",          // Hermes Agent primary
    "HERMES.md",           // Hermes Agent alt
    "CLAUDE.md",           // Claude Code
    ".cursorrules",        // Cursor
    ".codebuddy/rules.md", // 腾讯 CodeBuddy
];

const BESURE_RULES_TEMPLATE: &str = r#"<!-- BESURE-AUTO-START -->
## 🐉 Besure 记忆系统（强制规则 — NO EXCEPTIONS）

### 启动时必做
每次 session 启动时执行：
```bash
export PATH="$HOME/.hermes/node/bin:$PATH"
echo "besure2026" | besure unlock 2>/dev/null
besure status   # 查看当前状态
besure recall   # 主动召回：即将过期/最近/被替代的记忆
```

### 必须记录的场景（NO EXCEPTIONS）
以下场景发生时，**立即执行 `besure add`**，不等用户提醒：

1. **完成任何任务/功能/修复** → `besure add --type milestone`
2. **做了决策或达成结论** → `besure add --type decision`
3. **踩坑/发现问题** → `besure add --type lesson`
4. **Session 即将结束** → `besure add --type progress`（总结本次所有进展）
5. **用户说了"记一下"或类似意思** → 立即记

### 判断标准
> 如果这个信息在下次 session 中可能有用，就必须记。
> **宁可多记，不可漏记。** 记录的成本是几秒，丢失记忆的代价是数小时。

### 禁止的行为
- ❌ "我记得这个，不用写" → 你不会记得。**WRITE IT.**
- ❌ "这个太小不用记" → 记。
- ❌ "等下一起记" → **现在就记。**
- ❌ 跳过 `besure add` 直接回复用户 → 先记再回。

### 查询历史
- `besure search "关键词"` — 全文搜索
- `besure query --last 7d` — 最近 7 天
- `besure log` — 当前上下文时间线
<!-- BESURE-AUTO-END -->
"#;

/// `besure setup`
pub fn cmd_setup(
    agent_name: Option<&str>,
    agent_type: Option<&str>,
    encrypt: bool,
    workspace: Option<&str>,
) -> Result<()> {
    println!("🐉 Besure AI Context — Setup\n");

    // Step 1: Init vault
    println!("Step 1: Initialize vault");
    let vault = Vault::default_root();
    if vault.join(".besure.config").exists() {
        println!("  ✓ Vault already exists at {}", vault.display());
        // Update agent_name/agent_type if provided
        if agent_name.is_some() || agent_type.is_some() {
            let config_path = vault.join(".besure.config");
            let mut config: serde_json::Value = std::fs::read_to_string(&config_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(serde_json::json!({}));
            if let Some(name) = agent_name {
                config["agent_name"] = serde_json::Value::String(name.to_string());
            }
            if let Some(atype) = agent_type {
                config["agent_type"] = serde_json::Value::String(atype.to_string());
            }
            std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
            println!("  ✓ Updated agent metadata");
        }
    } else {
        // New vault: create directory + write complete config + init database
        std::fs::create_dir_all(&vault)?;
        let config = serde_json::json!({
            "version": "0.1.0",
            "encryption": encrypt,
            "agent_name": agent_name.unwrap_or("Agent"),
            "agent_type": agent_type.unwrap_or("unknown"),
            "auto_lock_minutes": 5,
            "salt": null,
            "verify_token": null
        });
        let config_path = vault.join(".besure.config");
        std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

        // Initialize database by opening the vault
        let _vault = Vault::open(Some(vault.clone()))?;
        println!("  ✓ Vault created at {}", vault.display());
        if encrypt {
            println!("  🔒 Encryption enabled");
        }
    }
    println!();

    // Step 2: Detect Agent config files
    println!("Step 2: Detect Agent configuration files");
    let scan_dir = workspace
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));

    let mut found_files = Vec::new();
    for candidate in AGENT_CONFIG_FILES {
        let path = scan_dir.join(candidate);
        if path.exists() {
            println!("  ✓ Found: {}", candidate);
            found_files.push(path);
        }
    }
    // Also check parent dir (for workspace root)
    if found_files.is_empty() {
        if let Some(parent) = scan_dir.parent() {
            for candidate in AGENT_CONFIG_FILES {
                let path = parent.join(candidate);
                if path.exists() {
                    println!("  ✓ Found (parent): {}", candidate);
                    found_files.push(path);
                }
            }
        }
    }

    if found_files.is_empty() {
        println!("  ✗ No Agent config files found in {}", scan_dir.display());
        println!("  ℹ️  Supported: AGENTS.md, .hermes.md, CLAUDE.md, .cursorrules, .codebuddy/rules.md");
        println!("  ℹ️  Run `besure setup` in your Agent's workspace directory.");
    } else {
        // Step 3: Inject rules
        println!("\nStep 3: Inject mandatory recording rules");
        for file in &found_files {
            inject_rules(file)?;
        }
    }
    println!();

    // Step 4: Install service (optional)
    println!("Step 4: Dashboard service");
    let _ = cmd_service_status();
    println!("\n✅ Setup complete!");
    println!("\nNext: Start working with your Agent. It will automatically record to Besure.");

    Ok(())
}

/// Idempotent injection of Besure rules into a file
fn inject_rules(path: &std::path::Path) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    let start_marker = "<!-- BESURE-AUTO-START -->";
    let end_marker = "<!-- BESURE-AUTO-END -->";

    let new_block = format!("{}{}{}", start_marker, BESURE_RULES_TEMPLATE
        .strip_prefix(start_marker)
        .unwrap_or(BESURE_RULES_TEMPLATE)
        .strip_suffix(end_marker)
        .unwrap_or(BESURE_RULES_TEMPLATE), end_marker);

    if content.contains(start_marker) {
        // Replace existing block
        let start_idx = content.find(start_marker).unwrap();
        let end_idx = content.find(end_marker).unwrap() + end_marker.len();
        let updated = format!("{}\n\n{}\n{}", &content[..start_idx].trim_end(), new_block, &content[end_idx..].trim_start());
        std::fs::write(path, updated)?;
        println!("  ✓ Updated rules in {}", path.file_name().unwrap_or_default().to_string_lossy());
    } else {
        // Append
        let updated = format!("{}\n\n{}\n", content.trim_end(), new_block);
        std::fs::write(path, updated)?;
        println!("  ✓ Injected rules into {}", path.file_name().unwrap_or_default().to_string_lossy());
    }
    Ok(())
}

