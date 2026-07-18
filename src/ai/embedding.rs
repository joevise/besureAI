use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Embedding 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub provider: String,      // "openai" | "minimax" | "dummy"
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    pub dimensions: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: "dummy".to_string(),
            api_url: String::new(),
            api_key: String::new(),
            model: "dummy".to_string(),
            dimensions: 64,
        }
    }
}

/// Embedding 提供者：调远程 API 或本地 dummy（开发用）
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
        let client = if config.provider != "dummy" {
            reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .ok()
        } else {
            None
        };
        Self { config, client }
    }

    /// 生成单条文本的 embedding
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        if self.config.provider == "dummy" {
            return Ok(Self::dummy_embed(text, self.config.dimensions));
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

    #[test]
    fn test_dummy_embed() {
        let provider = EmbeddingProvider::new(EmbeddingConfig::default());
        let v1 = provider.embed("hello world").unwrap();
        let v2 = provider.embed("hello world").unwrap();
        let v3 = provider.embed("different text").unwrap();

        assert_eq!(v1.len(), 64);
        assert_eq!(v1, v2); // 确定性
        assert_ne!(v1, v3); // 不同文本不同向量
    }

    #[test]
    fn test_dummy_normalized() {
        let provider = EmbeddingProvider::new(EmbeddingConfig::default());
        let v = provider.embed("test normalization").unwrap();
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01); // 归一化后 norm ≈ 1
    }

    #[test]
    fn test_batch() {
        let provider = EmbeddingProvider::new(EmbeddingConfig::default());
        let texts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let vecs = provider.embed_batch(&texts).unwrap();
        assert_eq!(vecs.len(), 3);
    }
}
