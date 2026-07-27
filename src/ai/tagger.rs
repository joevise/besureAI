use anyhow::Result;

use super::absorb::LlmConfig;

/// 自动标签器已下线：不再调用 LLM。
/// 保留类型与 API 形态（rest_api 等处仍在引用），tag() 恒返回空标签。
/// 标签改由 Agent 在 `besure add --tags` 时显式传入。
pub struct Tagger {
    _llm_config: LlmConfig,
}

impl Tagger {
    /// 从 ~/.besure/appconfig.json 的 llm 段构造 Tagger（REST 等非 CLI 路径用）
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
        Self { _llm_config: llm_config }
    }

    /// 已废弃：不再调用 LLM，恒返回空标签。
    pub fn tag(&self, _content: &str, _existing_tags: &[String]) -> Result<Vec<String>> {
        Ok(vec![])
    }

    /// 解析标签数组文本（容忍 ```json 包裹 / 前后杂文本）
    pub fn parse_tags(raw: &str) -> Vec<String> {
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
    fn test_tag_always_empty() {
        let tagger = Tagger::new(LlmConfig::default());
        let tags = tagger.tag("完成了后端 API 的部署", &["后端开发".to_string()]).unwrap();
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
