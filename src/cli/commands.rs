use anyhow::{bail, Context as AnyhowContext, Result};
use std::io::{self, Write};
use std::path::PathBuf;

use besure_lib::ai::{EmbeddingProvider, embedding::EmbeddingConfig, Absorber, absorb::LlmConfig, VectorStore};
use besure_lib::storage::{Vault, Context, Entry};

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
    let mut vault = get_vault()?;
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

    // 精确匹配
    if let Some(ctx) = db.get_context(query)? {
        vault.set_current(&ctx.id)?;
        println!("✓ Switched to: {} ({})", ctx.title, ctx.id);
        return Ok(());
    }

    // 模糊匹配
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
pub fn cmd_add_from_args(content: &str, entry_type: &str) -> Result<()> {
    let vault = get_unlocked_vault()?;
    let context_id = vault
        .current_context
        .as_ref()
        .context("No active context. Run 'besure create' or 'besure switch' first.")?;

    let entry = Entry::new(context_id, content, entry_type);

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
            println!(
                "┌─ [{}] {} ({})",
                entries.len() - i,
                entry.date,
                entry.entry_type
            );
            println!("│ {}", entry.content);
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

// === config ===
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
