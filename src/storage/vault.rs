use anyhow::{bail, Context as AnyhowContext, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::crypto::VaultCrypto;
use super::db::Database;
use super::models::{Context, Entry};

/// Vault 配置（不加密，存在 ~/.besure/.besure.config）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VaultConfig {
    pub version: String,
    pub encryption: bool,
    pub salt: Option<Vec<u8>>,
    pub verify_token: Option<Vec<u8>>,
    pub auto_lock_minutes: u32,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            version: "0.1.0".to_string(),
            encryption: true,
            salt: None,
            verify_token: None,
            auto_lock_minutes: 5,
        }
    }
}

/// Besure Vault — 核心存储管理器
///
/// 管理 ~/.besure/ 目录结构：
/// - besure.db (SQLite，加密时为 besure.db.enc)
/// - vault/ (Markdown 文件)
/// - .besure.config (配置)
pub struct Vault {
    pub root: PathBuf,
    pub vault_dir: PathBuf,
    pub db_path: PathBuf,
    pub config_path: PathBuf,
    pub config: VaultConfig,
    pub crypto: Option<VaultCrypto>,
    pub current_context: Option<String>,
}

impl Vault {
    /// 获取 vault 根目录（默认 ~/.besure/）
    pub fn default_root() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".besure")
    }

    /// 初始化新 vault
    pub fn init(root: Option<PathBuf>, password: Option<&str>) -> Result<Self> {
        let root = root.unwrap_or_else(Self::default_root);
        fs::create_dir_all(&root).context("failed to create vault root")?;
        let vault_dir = root.join("vault");
        fs::create_dir_all(&vault_dir).context("failed to create vault dir")?;

        let mut config = VaultConfig::default();

        // 加密设置
        let crypto = if let Some(_pw) = password {
            let mut c = VaultCrypto::new()?;
            c.unlock(_pw);
            config.salt = Some(c.salt().to_vec());
            config.verify_token = Some(c.generate_verify_token()?);
            Some(c)
        } else {
            config.encryption = false;
            None
        };

        let config_path = root.join(".besure.config");
        let config_json = serde_json::to_string_pretty(&config)?;
        fs::write(&config_path, config_json)?;

        // 创建数据库
        let db_path = root.join("besure.db");
        let _db = Database::open(&db_path)?;

        // 如果启用加密，加密初始数据库并删除明文
        if let Some(ref crypto) = crypto {
            if crypto.is_unlocked() {
                let plaintext = fs::read(&db_path)?;
                let ciphertext = crypto.encrypt(&plaintext)?;
                fs::write(root.join("besure.db.enc"), ciphertext)?;
                let _ = fs::remove_file(&db_path);
            }
        };

        Ok(Self {
            root,
            vault_dir,
            db_path,
            config_path,
            config,
            crypto,
            current_context: None,
        })
    }

    /// 打开已有 vault
    pub fn open(root: Option<PathBuf>) -> Result<Self> {
        let root = root.unwrap_or_else(Self::default_root);
        if !root.exists() {
            bail!("vault not found at {}. Run 'besure init' first.", root.display());
        }

        let config_path = root.join(".besure.config");
        let config_json = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config: {}", config_path.display()))?;
        let config: VaultConfig = serde_json::from_str(&config_json)?;

        let vault_dir = root.join("vault");
        fs::create_dir_all(&vault_dir)?;

        let db_path = if config.encryption {
            root.join("besure.db.enc")
        } else {
            root.join("besure.db")
        };

        // 读取 current context
        let current_file = root.join(".current");
        let current_context = fs::read_to_string(&current_file).ok().filter(|s| !s.trim().is_empty());

        Ok(Self {
            root,
            vault_dir,
            db_path,
            config_path,
            config,
            crypto: None,
            current_context,
        })
    }

    /// 检查是否已初始化
    pub fn exists(root: Option<PathBuf>) -> bool {
        let root = root.unwrap_or_else(Self::default_root);
        root.join(".besure.config").exists()
    }

    /// 解锁 vault
    pub fn unlock(&mut self, password: &str) -> Result<bool> {
        if !self.config.encryption {
            return Ok(true); // 无加密，直接返回
        }

        let salt = self.config.salt.as_ref().context("no salt in config")?;
        let verify = self.config.verify_token.as_ref().context("no verify token")?;

        let mut crypto = VaultCrypto::from_salt(salt.clone());
        if !crypto.unlock_with_verify(password, verify)? {
            bail!("wrong password");
        }

        // 解密数据库到内存供使用
        let enc_db_path = self.root.join("besure.db.enc");
        if enc_db_path.exists() {
            let plaintext = crypto.decrypt_file(&enc_db_path)?;
            let plain_db = self.root.join("besure.db");
            fs::write(&plain_db, plaintext)?;
        }

        self.crypto = Some(crypto);
        Ok(true)
    }

    /// 锁定 vault
    pub fn lock(&mut self) -> Result<()> {
        if let Some(ref crypto) = self.crypto {
            // 将数据库加密写回
            let plain_db = self.root.join("besure.db");
            if plain_db.exists() {
                let plaintext = fs::read(&plain_db)?;
                let ciphertext = crypto.encrypt(&plaintext)?;
                let enc_path = self.root.join("besure.db.enc");
                fs::write(&enc_path, ciphertext)?;
                // 删除明文
                let _ = fs::remove_file(&plain_db);
            }
            self.crypto.as_mut().unwrap().lock();
        }
        self.crypto = None;
        Ok(())
    }

    /// 是否已解锁
    pub fn is_unlocked(&self) -> bool {
        if !self.config.encryption {
            return true;
        }
        // CLI 模式下，检查明文 .db 是否存在（unlock 写入，lock 删除）
        if self.crypto.as_ref().map(|c| c.is_unlocked()).unwrap_or(false) {
            return true;
        }
        // 明文 DB 存在 = 解锁状态（跨进程）
        self.root.join("besure.db").exists()
    }

    /// 获取数据库连接
    pub fn database(&self) -> Result<Database> {
        let db_path = if self.config.encryption {
            self.root.join("besure.db")
        } else {
            self.db_path.clone()
        };
        Database::open(&db_path)
    }

    /// 设置当前上下文
    pub fn set_current(&mut self, context_id: &str) -> Result<()> {
        self.current_context = Some(context_id.to_string());
        let current_file = self.root.join(".current");
        fs::write(&current_file, context_id)?;
        Ok(())
    }

    /// 清除当前上下文
    pub fn clear_current(&mut self) -> Result<()> {
        self.current_context = None;
        let current_file = self.root.join(".current");
        let _ = fs::remove_file(current_file);
        Ok(())
    }

    /// 创建上下文目录（Markdown 文件存储）
    pub fn context_dir(&self, context_id: &str) -> PathBuf {
        self.vault_dir.join(context_id)
    }

    /// 写入上下文的 Markdown 文件
    pub fn write_context_md(&self, ctx: &Context) -> Result<()> {
        let dir = self.context_dir(&ctx.id);
        fs::create_dir_all(&dir)?;
        fs::create_dir_all(dir.join("entries"))?;

        // CONTEXT.md
        let context_md = ctx.to_context_md();
        self.write_file(&dir.join("CONTEXT.md"), context_md.as_bytes())?;

        // meta.json
        let meta = ctx.to_meta_json();
        self.write_file(&dir.join("meta.json"), meta.as_bytes())?;

        Ok(())
    }

    /// 写入 entry 的 Markdown 文件
    pub fn write_entry_md(&self, entry: &Entry) -> Result<()> {
        let ctx_dir = self.context_dir(&entry.context_id);
        let entries_dir = ctx_dir.join("entries");
        fs::create_dir_all(&entries_dir)?;

        // 用 entry id 里的时间戳部分作为文件名序号
        let filename = format!("{}.md", entry.id.split('_').last().unwrap_or("0"));
        let md = entry.to_markdown();
        self.write_file(&entries_dir.join(&filename), md.as_bytes())?;

        Ok(())
    }

    /// 写文件（加密模式下加密写入）
    fn write_file(&self, path: &Path, content: &[u8]) -> Result<()> {
        if let Some(ref crypto) = self.crypto {
            if crypto.is_unlocked() {
                let enc = crypto.encrypt(content)?;
                let enc_path = format!("{}.enc", path.display());
                let mut file = fs::File::create(&enc_path)?;
                file.write_all(&enc)?;
                return Ok(());
            }
        }
        // 明文写入
        let mut file = fs::File::create(path)?;
        file.write_all(content)?;
        Ok(())
    }

    /// 导出上下文为单个 Markdown 文件
    pub fn export_context(&self, ctx: &Context, entries: &[Entry], output: &Path) -> Result<()> {
        let mut md = ctx.to_context_md();

        md.push_str("## 进展时间线\n\n");
        for entry in entries.iter().rev() {
            md.push_str(&format!(
                "### {} ({})\n{}\n\n",
                entry.date, entry.entry_type, entry.content
            ));
        }

        md.push_str("---\n*Exported from Besure AI — 貔貅记忆*\n");
        fs::write(output, md)?;
        Ok(())
    }

    /// 获取 vault 状态摘要
    pub fn status_summary(&self) -> Result<(i64, i64, Option<String>)> {
        let db = self.database()?;
        let ctx_count = db.count_contexts()?;
        let entry_count = db.count_entries()?;
        Ok((ctx_count, entry_count, self.current_context.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("besure_test_{}_{}", std::process::id(), rand::random::<u32>()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn test_init_and_open_no_encryption() {
        let dir = tmp_dir();

        // init without encryption
        let vault = Vault::init(Some(dir.clone()), None).unwrap();
        assert!(Vault::exists(Some(dir.clone())));

        // reopen
        let vault2 = Vault::open(Some(dir.clone())).unwrap();
        assert!(!vault2.config.encryption);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_init_and_unlock_with_encryption() {
        let dir = tmp_dir();

        let vault = Vault::init(Some(dir.clone()), Some("mypassword")).unwrap();
        drop(vault);

        let mut vault2 = Vault::open(Some(dir.clone())).unwrap();
        assert!(!vault2.is_unlocked());

        let ok = vault2.unlock("mypassword").unwrap();
        assert!(ok);
        assert!(vault2.is_unlocked());

        vault2.lock().unwrap();
        assert!(!vault2.is_unlocked());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_wrong_password() {
        let dir = tmp_dir();

        let vault = Vault::init(Some(dir.clone()), Some("correct")).unwrap();
        drop(vault);

        let mut vault2 = Vault::open(Some(dir.clone())).unwrap();
        let result = vault2.unlock("wrong");
        assert!(result.is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_create_and_switch_context() {
        let dir = tmp_dir();

        let mut vault = Vault::init(Some(dir.clone()), None).unwrap();

        let ctx = Context::from_title("Test Project");
        let db = vault.database().unwrap();
        db.upsert_context(&ctx).unwrap();
        vault.write_context_md(&ctx).unwrap();
        vault.set_current(&ctx.id).unwrap();

        assert_eq!(vault.current_context, Some(ctx.id));

        let _ = fs::remove_dir_all(&dir);
    }
}
