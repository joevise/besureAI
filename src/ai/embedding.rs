use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Embedding 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(default = "default_provider")]
    pub provider: String,      // "local" | "openai" | "minimax" | "dummy"
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_dimensions")]
    pub dimensions: usize,
}

fn default_provider() -> String { "local".to_string() }
fn default_model() -> String { "bge-small-zh-v1.5".to_string() }
fn default_dimensions() -> usize { 512 }

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: "local".to_string(),
            api_url: String::new(),
            api_key: String::new(),
            model: "bge-small-zh-v1.5".to_string(),
            dimensions: 512,
        }
    }
}

/// 全局缓存 fastembed TextEmbedding 实例（模型加载慢 ~1-2s，模型无状态可跨 vault 复用）。
/// 加载失败不缓存，下次调用可重试（如首次离线无法下载）。
fn local_model() -> Result<Arc<fastembed::TextEmbedding>> {
    static MODEL: OnceLock<Mutex<Option<Arc<fastembed::TextEmbedding>>>> = OnceLock::new();
    let cell = MODEL.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().map_err(|_| anyhow::anyhow!("embedding model lock poisoned"))?;
    if let Some(m) = guard.as_ref() {
        return Ok(m.clone());
    }
    let model = fastembed::TextEmbedding::try_new(
        fastembed::InitOptions::new(fastembed::EmbeddingModel::BGESmallZHV15)
            .with_show_download_progress(false),
    )
    .context("failed to load local embedding model (bge-small-zh-v1.5)")?;
    let model = Arc::new(model);
    *guard = Some(model.clone());
    Ok(model)
}

/// Embedding 提供者：本地 fastembed（默认）/ 远程 OpenAI 兼容 API / dummy（测试用）
pub struct EmbeddingProvider {
    config: EmbeddingConfig,
    client: Option<reqwest::blocking::Client>,
}

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

impl EmbeddingProvider {
    pub fn new(config: EmbeddingConfig) -> Self {
        let client = if config.provider != "dummy" && config.provider != "local" {
            reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .ok()
        } else {
            None
        };
        Self { config, client }
    }

    /// 从 ~/.besure/appconfig.json 的 embedding 段构造（MCP/REST 等非 CLI 路径用）
    pub fn from_app_config() -> Self {
        let path = crate::storage::Vault::default_root().join("appconfig.json");
        let config = std::fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
            .and_then(|v| v.get("embedding").cloned())
            .and_then(|e| serde_json::from_value::<EmbeddingConfig>(e).ok())
            .unwrap_or_default();
        Self::new(config)
    }

    /// 当前配置的向量维度
    pub fn dimensions(&self) -> usize {
        self.config.dimensions
    }

    /// 生成单条文本的 embedding
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        if self.config.provider == "dummy" {
            return Ok(Self::dummy_embed(text, self.config.dimensions));
        }
        if self.config.provider == "local" {
            let model = local_model()?;
            let mut embs = model
                .embed(vec![text], None)
                .context("local embedding failed")?;
            return embs
                .pop()
                .context("no embedding returned from local model");
        }

        let client = self.client.as_ref().context("no HTTP client")?;
        let req = EmbeddingRequest {
            model: self.config.model.clone(),
            input: text.to_string(),
        };

        let resp = client
            .post(&self.config.api_url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&req)
            .send()
            .context("embedding API request failed")?;

        let emb: EmbeddingResponse = resp
            .json()
            .context("embedding API parse failed")?;

        emb.data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .context("no embedding in response")
    }

    /// 批量生成
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if self.config.provider == "dummy" {
            return Ok(texts
                .iter()
                .map(|t| Self::dummy_embed(t, self.config.dimensions))
                .collect());
        }
        if self.config.provider == "local" {
            let model = local_model()?;
            let batch: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
            return model
                .embed(batch, None)
                .context("local embedding failed");
        }
        // 逐条调（简单实现，API 支持批量时可优化）
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Dummy embedding：基于文本 hash 的确定性伪向量（开发/测试用）
    fn dummy_embed(text: &str, dimensions: usize) -> Vec<f32> {
        let bytes = text.as_bytes();
        let mut vec = vec![0.0f32; dimensions];
        for (i, &b) in bytes.iter().enumerate() {
            vec[i % dimensions] += (b as f32) / 255.0;
        }
        // 归一化
        let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }
        vec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_config() -> EmbeddingConfig {
        EmbeddingConfig {
            provider: "dummy".to_string(),
            api_url: String::new(),
            api_key: String::new(),
            model: "dummy".to_string(),
            dimensions: 64,
        }
    }

    #[test]
    fn test_default_is_local() {
        let cfg = EmbeddingConfig::default();
        assert_eq!(cfg.provider, "local");
        assert_eq!(cfg.model, "bge-small-zh-v1.5");
        assert_eq!(cfg.dimensions, 512);
    }

    #[test]
    fn test_dummy_embed() {
        let provider = EmbeddingProvider::new(dummy_config());
        let v1 = provider.embed("hello world").unwrap();
        let v2 = provider.embed("hello world").unwrap();
        let v3 = provider.embed("different text").unwrap();

        assert_eq!(v1.len(), 64);
        assert_eq!(v1, v2); // 确定性
        assert_ne!(v1, v3); // 不同文本不同向量
    }

    #[test]
    fn test_dummy_normalized() {
        let provider = EmbeddingProvider::new(dummy_config());
        let v = provider.embed("test normalization").unwrap();
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01); // 归一化后 norm ≈ 1
    }

    #[test]
    fn test_batch() {
        let provider = EmbeddingProvider::new(dummy_config());
        let texts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let vecs = provider.embed_batch(&texts).unwrap();
        assert_eq!(vecs.len(), 3);
    }
}
