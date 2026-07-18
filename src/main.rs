mod crypto;
mod storage;
mod ai;
mod dashboard;
mod cli;

use clap::{ArgAction, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "besure",
    about = "貔貅记忆 Besure AI — Context Switch Memory System",
    version = "0.2.0",
    long_about = "本地优先多上下文记忆系统 — 只进不出，记忆永存。"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 初始化 vault
    #[command(name = "init")]
    Init {
        #[arg(long, action = ArgAction::SetTrue)]
        encrypt: bool,
    },

    /// 创建新上下文
    #[command(name = "create")]
    Create {
        title: String,
        #[arg(long = "tag", action = ArgAction::Append)]
        tags: Vec<String>,
        #[arg(long)]
        summary: Option<String>,
    },

    /// 切换到某上下文
    #[command(name = "switch")]
    Switch {
        query: String,
    },

    /// 添加进展记录
    #[command(name = "add")]
    Add {
        content: String,
        #[arg(short = 't', long = "type", default_value = "progress")]
        entry_type: String,
    },

    /// 列出所有上下文
    #[command(name = "list")]
    List,

    /// 查看上下文时间线
    #[command(name = "log")]
    Log {
        context: Option<String>,
    },

    /// 搜索（全文匹配 + 语义搜索）
    #[command(name = "search")]
    Search {
        query: String,
        /// 使用语义搜索（需配置 embedding）
        #[arg(long, action = ArgAction::SetTrue)]
        semantic: bool,
    },

    /// 导出上下文
    #[command(name = "export")]
    Export {
        context: String,
        #[arg(short = 'o', long = "output")]
        output: Option<String>,
    },

    /// 解锁 vault
    #[command(name = "unlock")]
    Unlock,

    /// 锁定 vault
    #[command(name = "lock")]
    Lock,

    /// 查看 vault 状态
    #[command(name = "status")]
    Status,

    /// 自动提取进展（从 stdin 或文件）
    #[command(name = "absorb")]
    Absorb {
        /// 从文件读取（不指定则读 stdin）
        #[arg(long)]
        from: Option<String>,
        /// 自动添加到当前上下文
        #[arg(long, action = ArgAction::SetTrue)]
        auto: bool,
    },

    /// 启动 MCP Server (stdio)
    #[command(name = "mcp")]
    Mcp,

    /// 启动 REST API 服务器
    #[command(name = "serve")]
    Serve {
        #[arg(long, default_value = "7788")]
        port: u16,
    },

    /// 配置管理
    #[command(name = "config")]
    Config {
        /// set key value
        key: String,
        value: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { encrypt } => cli::commands::cmd_init_from_args(encrypt),
        Commands::Create { title, tags, summary } => {
            cli::commands::cmd_create_from_args(&title, &tags, summary.as_deref())
        }
        Commands::Switch { query } => cli::commands::cmd_switch_from_args(&query),
        Commands::Add { content, entry_type } => {
            cli::commands::cmd_add_from_args(&content, &entry_type)
        }
        Commands::List => cli::commands::cmd_list(),
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
        Commands::Config { key, value } => {
            cli::commands::cmd_config_set(&key, &value)
        }
    }
}
