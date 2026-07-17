mod crypto;
mod storage;
mod cli;

use clap::{ArgAction, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "besure",
    about = "貔貅记忆 Besure AI — Context Switch Memory System",
    version = "0.1.0",
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
        /// 启用加密
        #[arg(long, action = ArgAction::SetTrue)]
        encrypt: bool,
    },

    /// 创建新上下文
    #[command(name = "create")]
    Create {
        /// 上下文标题
        title: String,

        /// 标签（可多次使用）
        #[arg(long = "tag", action = ArgAction::Append)]
        tags: Vec<String>,

        /// 摘要
        #[arg(long)]
        summary: Option<String>,
    },

    /// 切换到某上下文（支持模糊匹配）
    #[command(name = "switch")]
    Switch {
        /// 上下文 ID 或关键词
        query: String,
    },

    /// 添加进展记录到当前上下文
    #[command(name = "add")]
    Add {
        /// 内容
        content: String,

        /// 记录类型：init/milestone/decision/progress/blocker/note
        #[arg(short = 't', long = "type", default_value = "progress")]
        entry_type: String,
    },

    /// 列出所有上下文
    #[command(name = "list")]
    List,

    /// 查看上下文时间线
    #[command(name = "log")]
    Log {
        /// 指定上下文（不指定则用当前）
        context: Option<String>,
    },

    /// 搜索（全文匹配，V1 升级为语义搜索）
    #[command(name = "search")]
    Search {
        /// 搜索关键词
        query: String,
    },

    /// 导出上下文为 Markdown 文件
    #[command(name = "export")]
    Export {
        /// 上下文 ID 或关键词
        context: String,

        /// 输出文件路径
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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { encrypt } => {
            cli::commands::cmd_init_from_args(encrypt)
        }
        Commands::Create { title, tags, summary } => {
            cli::commands::cmd_create_from_args(&title, &tags, summary.as_deref())
        }
        Commands::Switch { query } => {
            cli::commands::cmd_switch_from_args(&query)
        }
        Commands::Add { content, entry_type } => {
            cli::commands::cmd_add_from_args(&content, &entry_type)
        }
        Commands::List => {
            cli::commands::cmd_list()
        }
        Commands::Log { context } => {
            cli::commands::cmd_log_from_args(context.as_deref())
        }
        Commands::Search { query } => {
            cli::commands::cmd_search_from_args(&query)
        }
        Commands::Export { context, output } => {
            cli::commands::cmd_export_from_args(&context, output.as_deref())
        }
        Commands::Unlock => {
            cli::commands::cmd_unlock()
        }
        Commands::Lock => {
            cli::commands::cmd_lock()
        }
        Commands::Status => {
            cli::commands::cmd_status()
        }
    }
}
