use anyhow::Result;
use std::time::Duration;

use super::absorb::LlmConfig;

/// 自动标签器：给 entry 内容打 1-3 个扁平大类中文标签。
/// 标签库（tag_vocab）用于复用已有标签，避免同义词爆炸。
pub struct Tagger {
    llm_config: LlmConfig,
    client: Option<reqwest::blocking::Client>,
}

const TAG_PROMPT: &str = r#"你是一个内容分类助手。给给定内容打 1-3 个「大类」中文标签。

规则：
1. 标签必须是宽泛的大类（如：后端开发、前端开发、部署、数据库、家庭、投资、健康、学习、产品规划），不要具体名词
2. 优先从下方提供的「已有标签库」中选择，语义相同或相近就复用已有标签，绝对不要造同义词（如已有「后端开发」就不要新建「后端」）
3. 只有当已有标签库里确实没有合适标签时，才创建新标签
4. 最多 3 个标签，最少 1 个
5. 只输出 JSON 字符串数组，不要任何其他文字

输出格式：["标签1", "标签2"]"#;

impl Tagger {
    /// 从 ~/.besure/appconfig.json 的 llm 段构造 Tagger（MCP/REST 等非 CLI 路径用）
    pub fn from_app_config() -> Self {
        let path = crate::storage::Vault::default_root().join("appconfig.json");
        let llm_config = std::fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
            .and_then(|v| v.get("llm").cloned())
            .and_then(|llm| serde_json::from_value::<LlmConfig>(llm).ok())
            .unwrap_or_default();
        Self::new(llm_config)
    }

    pub fn new(llm_config: LlmConfig) -> Self {
        let client = if llm_config.provider != "dummy" && !llm_config.api_url.is_empty() {
            reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .ok()
        } else {
            None
        };
        Self { llm_config, client }
    }

    /// 给一条内容打标签。existing_tags 是当前标签库（用于复用匹配）。
    /// LLM 不可用时降级返回 Ok(vec![])，绝不阻塞 add。
    pub fn tag(&self, content: &str, existing_tags: &[String]) -> Result<Vec<String>> {
        // 降级：dummy provider 或无 api_url
        if self.llm_config.provider == "dummy" || self.llm_config.api_url.is_empty() {
            return Ok(vec![]);
        }

        let result = self.tag_inner(content, existing_tags);
        match result {
            Ok(tags) => Ok(tags),
            Err(e) => {
                eprintln!("⚠️  auto-tagging failed (degraded, entry saved without tags): {}", e);
                Ok(vec![])
            }
        }
    }

    fn tag_inner(&self, content: &str, existing_tags: &[String]) -> Result<Vec<String>> {
        let client = match self.client.as_ref() {
            Some(c) => c,
            None => return Ok(vec![]),
        };

        let vocab_hint = if existing_tags.is_empty() {
            "（已有标签库为空）".to_string()
        } else {
            format!("已有标签库：{}", existing_tags.join("、"))
        };

        let user_msg = format!("{}\n\n内容：\n{}", vocab_hint, content);

        let req = serde_json::json!({
            "model": &self.llm_config.model,
            "messages": [
                {"role": "system", "content": TAG_PROMPT},
                {"role": "user", "content": user_msg}
            ],
            "temperature": 1.0,
        });

        let resp = client
            .post(&self.llm_config.api_url)
            .header("Authorization", format!("Bearer {}", self.llm_config.api_key))
            .json(&req)
            .send()?;

        let resp_json: serde_json::Value = resp.json()?;
        let raw = resp_json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("[]");

        let tags = Self::parse_tags(raw);
        Ok(tags)
    }

    /// 解析 LLM 输出为标签数组（容忍 ```json 包裹 / 前后杂文本）
    fn parse_tags(raw: &str) -> Vec<String> {
        let trimmed = raw.trim();
        // 提取第一个 [...] 区间
        let json_str = if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.rfind(']')) {
            if start < end {
                &trimmed[start..=end]
            } else {
                trimmed
            }
        } else {
            trimmed
        };

        let tags: Vec<String> = serde_json::from_str(json_str).unwrap_or_default();
        tags.into_iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .take(3)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dummy_degrades_to_empty() {
        let tagger = Tagger::new(LlmConfig::default());
        let tags = tagger.tag("完成了后端 API 的部署", &["后端开发".to_string()]).unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn test_empty_api_url_degrades_to_empty() {
        let cfg = LlmConfig {
            provider: "openai".to_string(),
            api_url: String::new(),
            api_key: "sk-test".to_string(),
            model: "test".to_string(),
        };
        let tagger = Tagger::new(cfg);
        let tags = tagger.tag("任意内容", &[]).unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_tags_plain() {
        let tags = Tagger::parse_tags(r#"["后端开发", "部署"]"#);
        assert_eq!(tags, vec!["后端开发", "部署"]);
    }

    #[test]
    fn test_parse_tags_with_code_fence() {
        let raw = "```json\n[\"投资\", \"家庭\"]\n```";
        let tags = Tagger::parse_tags(raw);
        assert_eq!(tags, vec!["投资", "家庭"]);
    }

    #[test]
    fn test_parse_tags_invalid() {
        let tags = Tagger::parse_tags("这不是 JSON");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_tags_truncates_to_three() {
        let tags = Tagger::parse_tags(r#"["a", "b", "c", "d"]"#);
        assert_eq!(tags.len(), 3);
    }
}
