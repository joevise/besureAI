use besure_lib::crypto;
use besure_lib::storage;
use besure_lib::ai;
use besure_lib::dashboard;
mod cli;

use clap::{ArgAction, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "besure",
    about = "貔貅记忆 Besure AI — Context Switch Memory System",
    version = "0.58.0",
    long_about = "本地优先多上下文记忆系统 — 只进不出，记忆永存。"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize vault
    #[command(name = "init")]
    Init {
        #[arg(long, action = ArgAction::SetTrue)]
        encrypt: bool,
    },

    /// Create a new context
    #[command(name = "create")]
    Create {
        title: String,
        #[arg(long = "tag", action = ArgAction::Append)]
        tags: Vec<String>,
        #[arg(long)]
        summary: Option<String>,
    },

    /// Switch to a context
    #[command(name = "switch")]
    Switch {
        query: String,
    },

    /// Add a progress entry
    #[command(name = "add")]
    Add {
        /// Content text (alternative: --from-file)
        content: Option<String>,
        /// Read multi-paragraph content from file (Markdown)
        #[arg(long = "from-file")]
        from_file: Option<String>,
        #[arg(short = 't', long = "type", default_value = "progress")]
        entry_type: String,
        /// Explicitly specify target context ID (prevents cross-context data corruption)
        #[arg(long = "context")]
        context: Option<String>,
    },

    /// List all contexts
    #[command(name = "list")]
    List,

    /// Show the auto-tag vocabulary (tag + usage count)
    #[command(name = "tags")]
    Tags,

    /// Re-tag existing entries with the LLM tagger
    #[command(name = "retag")]
    Retag {
        /// Retag entries across all contexts
        #[arg(long, action = ArgAction::SetTrue)]
        all: bool,
        /// Retag a specific context (default: current context)
        #[arg(long = "context")]
        context: Option<String>,
    },

    /// View context timeline
    #[command(name = "log")]
    Log {
        context: Option<String>,
    },

    /// Search (text match + semantic search)
    #[command(name = "search")]
    Search {
        query: String,
        /// Use semantic search (requires embedding config)
        #[arg(long, action = ArgAction::SetTrue)]
        semantic: bool,
    },

    /// Export a context
    #[command(name = "export")]
    Export {
        context: String,
        #[arg(short = 'o', long = "output")]
        output: Option<String>,
    },

    /// Unlock vault
    #[command(name = "unlock")]
    Unlock,

    /// Lock vault
    #[command(name = "lock")]
    Lock,

    /// View vault status
    #[command(name = "status")]
    Status,

    /// Auto-extract progress from text (stdin or file)
    #[command(name = "absorb")]
    Absorb {
        /// Read from file (default: stdin)
        #[arg(long)]
        from: Option<String>,
        /// Auto-add to current context
        #[arg(long, action = ArgAction::SetTrue)]
        auto: bool,
    },

    /// Start MCP Server (stdio)
    #[command(name = "mcp")]
    Mcp,

    /// Start REST API / Dashboard server
    #[command(name = "serve")]
    Serve {
        #[arg(long, default_value = "7788")]
        port: u16,
    },

    /// App-level config (embedding/llm settings)
    #[command(name = "appconfig")]
    AppConfig {
        /// config key (e.g. embedding.provider)
        key: String,
        value: String,
    },

    /// Link an entry to another entry
    #[command(name = "link")]
    Link {
        entry_id: String,
        #[arg(long)]
        to: String,
        #[arg(long = "as")]
        as_relation: Option<String>,
    },

    /// Mark an entry as expired
    #[command(name = "expire")]
    Expire {
        entry_id: String,
    },

    /// Supersede an old entry with a new one
    #[command(name = "supersede")]
    Supersede {
        old_id: String,
        new_id: String,
    },

    /// Recall entries that need attention (expiring, overdue, recent)
    #[command(name = "recall")]
    Recall,

    /// Unified query over entries (time/type/keyword/resolved filters)
    #[command(name = "query")]
    Query {
        /// Last N days (e.g. --last 7d)
        #[arg(long)]
        last: Option<String>,
        /// From date YYYY-MM-DD
        #[arg(long)]
        from: Option<String>,
        /// To date YYYY-MM-DD
        #[arg(long)]
        to: Option<String>,
        /// Filter by entry type (repeatable)
        #[arg(long = "type", action = ArgAction::Append)]
        entry_types: Vec<String>,
        /// Search across all contexts
        #[arg(long, action = ArgAction::SetTrue)]
        all: bool,
        /// Query a specific context (no switch)
        #[arg(long)]
        context: Option<String>,
        /// Keyword filter on content
        #[arg(long)]
        keyword: Option<String>,
        /// Only unresolved entries
        #[arg(long, action = ArgAction::SetTrue)]
        unresolved: bool,
        /// Only resolved entries
        #[arg(long, action = ArgAction::SetTrue)]
        resolved: bool,
        /// Max results (default 20)
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Query across all vaults (requires BESURE_VAULTS_ALL=true)
        #[arg(long, action = ArgAction::SetTrue)]
        all_vaults: bool,
    },

    /// Mark an entry as resolved
    #[command(name = "resolve")]
    Resolve {
        entry_id: String,
    },

    /// Append content to an existing entry
    #[command(name = "append")]
    Append {
        entry_id: String,
        /// Content text (alternative: --from-file)
        content: Option<String>,
        /// Read content from file
        #[arg(long = "from-file")]
        from_file: Option<String>,
    },

    /// Statistics overview
    #[command(name = "stats")]
    Stats,

    /// List all vaults (requires BESURE_VAULTS_ALL=true)
    #[command(name = "vaults")]
    Vaults,

    /// Share an entry to the shared vault
    #[command(name = "share")]
    Share {
        entry_id: String,
    },

    /// Share an entire context to the shared vault
    #[command(name = "share-context")]
    ShareContext {
        context_id: String,
    },

    /// View shared vault contents
    #[command(name = "shared")]
    Shared {
        #[arg(long)]
        keyword: Option<String>,
        #[arg(long = "type", action = ArgAction::Append)]
        entry_types: Vec<String>,
        #[arg(long, default_value = "20")]
        limit: usize,
    },

    /// Install/uninstall Dashboard service (auto-start on boot)
    #[command(name = "service")]
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },

    /// Setup vault + inject mandatory recording rules into Agent config files
    #[command(name = "setup")]
    Setup {
        /// Agent name for this vault (e.g. "Joey", "CodeBuddy")
        #[arg(long)]
        agent_name: Option<String>,
        /// Agent type (openclaw/cursor/codex/claude-code/codebuddy/workbuddy/custom)
        #[arg(long)]
        agent_type: Option<String>,
        /// Encrypt vault
        #[arg(long, action = ArgAction::SetTrue)]
        encrypt: bool,
        /// Path to scan for Agent config files (default: current dir)
        #[arg(long)]
        workspace: Option<String>,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// Install Dashboard service (auto-start on boot, auto-restart on crash)
    Install,
    /// Uninstall Dashboard service
    Uninstall,
    /// Check service status
    Status,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { encrypt } => cli::commands::cmd_init_from_args(encrypt),
        Commands::Create { title, tags, summary } => {
            cli::commands::cmd_create_from_args(&title, &tags, summary.as_deref())
        }
        Commands::Switch { query } => cli::commands::cmd_switch_from_args(&query),
        Commands::Add { content, from_file, entry_type, context } => {
            cli::commands::cmd_add_from_args(content.as_deref(), from_file.as_deref(), &entry_type, context.as_deref())
        }
        Commands::List => cli::commands::cmd_list(),
        Commands::Tags => cli::commands::cmd_tags(),
        Commands::Retag { all, context } => {
            cli::commands::cmd_retag(all, context.as_deref())
        }
        Commands::Log { context } => cli::commands::cmd_log_from_args(context.as_deref()),
        Commands::Search { query, semantic } => {
            cli::commands::cmd_search_from_args(&query, semantic)
        }
        Commands::Export { context, output } => {
            cli::commands::cmd_export_from_args(&context, output.as_deref())
        }
        Commands::Unlock => cli::commands::cmd_unlock(),
        Commands::Lock => cli::commands::cmd_lock(),
        Commands::Status => cli::commands::cmd_status(),
        Commands::Absorb { from, auto } => {
            cli::commands::cmd_absorb_from_args(from.as_deref(), auto)
        }
        Commands::Mcp => {
            ai::mcp_server::McpServer::run()
        }
        Commands::Serve { port } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                ai::rest_api::ApiServer::new(port).run().await
            })
        }
        Commands::AppConfig { key, value } => {
            cli::commands::cmd_appconfig_set(&key, &value)
        }
        Commands::Link { entry_id, to, as_relation } => {
            cli::commands::cmd_link(&entry_id, &to, as_relation.as_deref())
        }
        Commands::Expire { entry_id } => {
            cli::commands::cmd_expire(&entry_id)
        }
        Commands::Supersede { old_id, new_id } => {
            cli::commands::cmd_supersede(&old_id, &new_id)
        }
        Commands::Recall => {
            cli::commands::cmd_recall()
        }
        Commands::Query { last, from, to, entry_types, all, context, keyword, unresolved, resolved, limit, all_vaults } => {
            if all_vaults {
                std::env::set_var("BESURE_QUERY_ALL_VAULTS", "1");
            }
            let args = cli::commands::QueryArgs {
                last, from, to, entry_types, all, context, keyword, unresolved, resolved, limit,
            };
            cli::commands::cmd_query(&args)
        }
        Commands::Resolve { entry_id } => {
            cli::commands::cmd_resolve(&entry_id)
        }
        Commands::Append { entry_id, content, from_file } => {
            cli::commands::cmd_append(&entry_id, content.as_deref(), from_file.as_deref())
        }
        Commands::Stats => {
            cli::commands::cmd_stats()
        }
        Commands::Vaults => {
            cli::commands::cmd_vaults()
        }
        Commands::Share { entry_id } => {
            cli::commands::cmd_share(&entry_id)
        }
        Commands::ShareContext { context_id } => {
            cli::commands::cmd_share_context(&context_id)
        }
        Commands::Shared { keyword, entry_types, limit } => {
            cli::commands::cmd_shared(keyword.as_deref(), &entry_types, limit)
        }
        Commands::Service { action } => {
            match action {
                ServiceAction::Install => cli::commands::cmd_service_install(),
                ServiceAction::Uninstall => cli::commands::cmd_service_uninstall(),
                ServiceAction::Status => cli::commands::cmd_service_status(),
            }
        }
        Commands::Setup { agent_name, agent_type, encrypt, workspace } => {
            cli::commands::cmd_setup(agent_name.as_deref(), agent_type.as_deref(), encrypt, workspace.as_deref())
        }
    }
}
