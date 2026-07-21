use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;

/// LLM 配置（用于 absorb 自动提取）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "llm_default_dummy")]
    pub provider: String,      // "openai" | "minimax" | "dummy"
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "llm_default_dummy")]
    pub model: String,
}

fn llm_default_dummy() -> String { "dummy".to_string() }

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "dummy".to_string(),
            api_url: String::new(),
            api_key: String::new(),
            model: "dummy".to_string(),
        }
    }
}

/// 自动提取器：从对话文本中提取结构化进展记录
pub struct Absorber {
    llm_config: LlmConfig,
    client: Option<reqwest::blocking::Client>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntry {
    pub content: String,
    pub entry_type: String,  // milestone/decision/progress/blocker/note
}

const ABSORB_PROMPT: &str = r#"你是一个进展提取助手。从给定的对话文本中，提取出关键进展记录。

规则：
1. 每条记录应该是一个独立的进展点（决策、完成、发现、阻碍等）
2. entry_type 从以下选一：milestone（里程碑）、decision（决策）、progress（进展）、blocker（阻碍）、note（备注）
3. content 用简洁的中文描述，一句话说清"做了什么/决定了什么/发现了什么"
4. 忽略寒暄、闲聊、重复内容
5. 最多提取 5 条最重要的

输出 JSON 数组格式：
[{"content": "完成了X", "entry_type": "progress"}, ...]

如果对话中没有实质进展，返回空数组 []。"#;

impl Absorber {
    pub fn new(llm_config: LlmConfig) -> Self {
        let client = if llm_config.provider != "dummy" {
            reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .ok()
        } else {
            None
        };
        Self { llm_config, client }
    }

    /// 从对话文本中提取进展记录
    pub fn absorb(&self, conversation: &str) -> Result<Vec<ExtractedEntry>> {
        if self.llm_config.provider == "dummy" {
            return Ok(Self::dummy_extract(conversation));
        }

        let client = self.client.as_ref().context("no HTTP client")?;

        let req = serde_json::json!({
            "model": &self.llm_config.model,
            "messages": [
                {"role": "system", "content": ABSORB_PROMPT},
                {"role": "user", "content": conversation}
            ],
            "temperature": 0.3,
        });

        let resp = client
            .post(&self.llm_config.api_url)
            .header("Authorization", format!("Bearer {}", self.llm_config.api_key))
            .json(&req)
            .send()
            .context("LLM API request failed")?;

        let resp_json: serde_json::Value = resp.json().context("LLM API parse failed")?;
        let content = resp_json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("[]");

        let entries: Vec<ExtractedEntry> = serde_json::from_str(content)
            .unwrap_or_default();

        Ok(entries)
    }

    /// 从 stdin 读取对话文本
    pub fn absorb_stdin(&self) -> Result<Vec<ExtractedEntry>> {
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        self.absorb(&input)
    }

    /// 从文件读取对话文本
    pub fn absorb_file(&self, path: &std::path::Path) -> Result<Vec<ExtractedEntry>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read: {}", path.display()))?;
        self.absorb(&content)
    }

    /// Dummy 提取（开发/测试用）：简单关键词匹配
    fn dummy_extract(text: &str) -> Vec<ExtractedEntry> {
        let mut entries = Vec::new();

        let patterns = [
            ("完成了", "progress"),
            ("做完了", "progress"),
            ("搞定了", "progress"),
            ("决定", "decision"),
            ("确定了", "decision"),
            ("选择", "decision"),
            ("里程碑", "milestone"),
            ("阶段", "milestone"),
            ("报错", "blocker"),
            ("失败", "blocker"),
            ("阻碍", "blocker"),
            ("卡住", "blocker"),
            ("注意", "note"),
            ("参考", "note"),
        ];

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.len() < 5 {
                continue;
            }
            for (keyword, entry_type) in &patterns {
                if line.contains(keyword) {
                    entries.push(ExtractedEntry {
                        content: line.to_string(),
                        entry_type: entry_type.to_string(),
                    });
                    break;
                }
            }
        }

        entries.truncate(5);
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dummy_extract() {
        let absorber = Absorber::new(LlmConfig::default());
        let text = r#"
        今天完成了加密引擎的开发
        遇到了一个报错，borrow checker 问题
        决定用 AES-256-GCM 而不是 ChaCha20
        注意：Argon2id 参数要调好
        一些无关的闲聊
        "#;
        let entries = absorber.absorb(text).unwrap();

        assert!(!entries.is_empty());
        assert!(entries.iter().any(|e| e.entry_type == "progress"));
        assert!(entries.iter().any(|e| e.entry_type == "blocker"));
        assert!(entries.iter().any(|e| e.entry_type == "decision"));
        assert!(entries.iter().any(|e| e.entry_type == "note"));
    }

    #[test]
    fn test_dummy_empty() {
        let absorber = Absorber::new(LlmConfig::default());
        let entries = absorber.absorb("hello\nworld\nhow are you").unwrap();
        assert!(entries.is_empty());
    }
}
