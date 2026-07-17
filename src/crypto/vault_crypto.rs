use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{bail, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use zeroize::Zeroize;

/// Argon2id 参数：64MB 内存、4 线程、3 次迭代
const ARGON2_MEMORY: u32 = 65536; // 64 MB in KB
const ARGON2_TIME: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;

/// Nonce 长度（AES-GCM 标准 96-bit / 12 bytes）
const NONCE_LEN: usize = 12;

/// 密钥长度（AES-256 = 32 bytes）
const KEY_LEN: usize = 32;

/// Salt 长度
const SALT_LEN: usize = 16;

/// 加密文件魔数（用于识别 besure 加密文件）
const MAGIC: &[u8] = b"BESURE1";

/// 保险箱加密引擎
///
/// 密钥只在内存中，lock() 或 Drop 时 zeroize。
/// 每个文件独立加密（nonce 唯一），AES-256-GCM 带认证标签。
pub struct VaultCrypto {
    salt: Vec<u8>,
    key: Option<[u8; KEY_LEN]>,
}

impl VaultCrypto {
    /// 创建新实例（初始化时用）：生成随机 salt
    pub fn new() -> Result<Self> {
        let mut salt = vec![0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        Ok(Self { salt, key: None })
    }

    /// 从已有 salt 加载（解锁时用）
    pub fn from_salt(salt: Vec<u8>) -> Self {
        Self { salt, key: None }
    }

    /// 获取 salt（用于持久化配置）
    pub fn salt(&self) -> &[u8] {
        &self.salt
    }

    /// 是否已解锁（密钥在内存中）
    pub fn is_unlocked(&self) -> bool {
        self.key.is_some()
    }

    /// Argon2id 密钥派生：密码 + salt → 256 位密钥
    fn derive_key(password: &str, salt: &[u8]) -> [u8; KEY_LEN] {
        let params = Params::new(ARGON2_MEMORY, ARGON2_PARALLELISM, ARGON2_TIME, Some(KEY_LEN))
            .expect("valid argon2 params");
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = [0u8; KEY_LEN];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .expect("argon2 derive failed");
        key
    }

    /// 解锁：验证密码并加载密钥到内存
    pub fn unlock(&mut self, password: &str) -> bool {
        let key = Self::derive_key(password, &self.salt);
        self.key = Some(key);
        true
    }

    /// 解锁并验证（用验证文件检查密码正确性）
    pub fn unlock_with_verify(&mut self, password: &str, verify_data: &[u8]) -> Result<bool> {
        let key = Self::derive_key(password, &self.salt);
        // 尝试解密验证数据
        match Self::decrypt_bytes(&key, verify_data) {
            Ok(_) => {
                self.key = Some(key);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// 锁定：从内存安全清除密钥
    pub fn lock(&mut self) {
        if let Some(ref mut key) = self.key {
            key.zeroize();
        }
        self.key = None;
    }

    /// 加密字节（返回：magic + nonce + ciphertext）
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let key = self.key.expect("vault locked");
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("AES-GCM encryption failed: {:?}", e))?;

        // 格式：MAGIC(7) + nonce(12) + ciphertext
        let mut result = Vec::with_capacity(MAGIC.len() + NONCE_LEN + ciphertext.len());
        result.extend_from_slice(MAGIC);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// 解密字节（输入：magic + nonce + ciphertext）
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        let key = self.key.expect("vault locked");
        Self::decrypt_bytes(&key, data)
    }

    fn decrypt_bytes(key: &[u8; KEY_LEN], data: &[u8]) -> Result<Vec<u8>> {
        // 检查最小长度
        if data.len() < MAGIC.len() + NONCE_LEN + 16 {
            bail!("data too short to be encrypted");
        }

        // 检查魔数
        if &data[..MAGIC.len()] != MAGIC {
            bail!("not a besure encrypted file (bad magic)");
        }

        let nonce_bytes = &data[MAGIC.len()..MAGIC.len() + NONCE_LEN];
        let ciphertext = &data[MAGIC.len() + NONCE_LEN..];

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|e| anyhow::anyhow!("AES-GCM decryption failed (wrong password or corrupted data): {:?}", e))?;
        Ok(plaintext)
    }

    /// 加密并写入文件（自动加 .enc 后缀）
    pub fn encrypt_file(&self, plaintext: &[u8], path: &std::path::Path) -> Result<()> {
        let ciphertext = self.encrypt(plaintext)?;
        let enc_path = format!("{}.enc", path.display());
        std::fs::write(&enc_path, ciphertext)?;
        Ok(())
    }

    /// 解密文件（读取 .enc 文件）
    pub fn decrypt_file(&self, path: &std::path::Path) -> Result<Vec<u8>> {
        let data = std::fs::read(path)?;
        self.decrypt(&data)
    }

    /// 生成验证令牌（初始化时用固定明文加密，之后用来验密码）
    pub fn generate_verify_token(&self) -> Result<Vec<u8>> {
        self.encrypt(b"BESURE_VERIFY_OK")
    }

    /// 验证密码是否正确（用验证令牌尝试解密）
    pub fn verify_password(&self, verify_token: &[u8]) -> Result<bool> {
        match self.decrypt(verify_token) {
            Ok(plain) => Ok(plain == b"BESURE_VERIFY_OK"),
            Err(_) => Ok(false),
        }
    }
}

impl Drop for VaultCrypto {
    fn drop(&mut self) {
        self.lock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_cycle() {
        let mut crypto = VaultCrypto::new().unwrap();
        crypto.unlock("test_password_123");

        let plaintext = "Hello, 貔貅记忆! This is a test.".as_bytes();
        let ciphertext = crypto.encrypt(plaintext).unwrap();
        let decrypted = crypto.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_wrong_password_fails() {
        let mut crypto1 = VaultCrypto::new().unwrap();
        crypto1.unlock("correct_password");

        let plaintext = b"Secret data";
        let ciphertext = crypto1.encrypt(plaintext).unwrap();

        let mut crypto2 = VaultCrypto::from_salt(crypto1.salt().to_vec());
        crypto2.unlock("wrong_password");

        let result = crypto2.decrypt(&ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_lock_clears_key() {
        let mut crypto = VaultCrypto::new().unwrap();
        crypto.unlock("password");
        assert!(crypto.is_unlocked());

        crypto.lock();
        assert!(!crypto.is_unlocked());
    }

    #[test]
    fn test_verify_token() {
        let mut crypto = VaultCrypto::new().unwrap();
        crypto.unlock("my_password");

        let token = crypto.generate_verify_token().unwrap();
        assert!(crypto.verify_password(&token).unwrap());

        let mut crypto2 = VaultCrypto::from_salt(crypto.salt().to_vec());
        crypto2.unlock("my_password");
        assert!(crypto2.verify_password(&token).unwrap());

        let mut crypto3 = VaultCrypto::from_salt(crypto.salt().to_vec());
        crypto3.unlock("wrong");
        assert!(!crypto3.verify_password(&token).unwrap());
    }

    #[test]
    fn test_file_encrypt_decrypt() {
        let mut crypto = VaultCrypto::new().unwrap();
        crypto.unlock("file_password");

        let plaintext = "File content test — 貔貅记忆".as_bytes();
        let tmp = std::env::temp_dir().join("besure_test_enc");
        crypto.encrypt_file(plaintext, &tmp).unwrap();

        let enc_path = format!("{}.enc", tmp.display());
        let decrypted = crypto.decrypt_file(std::path::Path::new(&enc_path)).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted);

        // cleanup
        let _ = std::fs::remove_file(enc_path);
    }

    #[test]
    fn test_magic_header() {
        let mut crypto = VaultCrypto::new().unwrap();
        crypto.unlock("pass");

        let ct = crypto.encrypt(b"data").unwrap();
        assert_eq!(&ct[..MAGIC.len()], MAGIC);
    }
}
