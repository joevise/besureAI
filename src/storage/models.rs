use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContextStatus {
    Active,
    Paused,
    Completed,
    Archived,
}

impl std::fmt::Display for ContextStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextStatus::Active => write!(f, "active"),
            ContextStatus::Paused => write!(f, "paused"),
            ContextStatus::Completed => write!(f, "completed"),
            ContextStatus::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for ContextStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(ContextStatus::Active),
            "paused" => Ok(ContextStatus::Paused),
            "completed" => Ok(ContextStatus::Completed),
            "archived" => Ok(ContextStatus::Archived),
            _ => Err(format!("unknown status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub id: String,
    pub title: String,
    pub status: ContextStatus,
    pub created: String,
    pub updated: String,
    pub tags: Vec<String>,
    pub summary: String,
    pub current_milestone: String,
    pub next_steps: Vec<String>,
    pub related: Vec<String>,
    pub shareable: bool,
}

impl Context {
    /// 从标题生成 context id（slugify）
    pub fn from_title(title: &str) -> Self {
        let id = slugify(title);
        let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
        Self {
            id: format!("ctx_{}", id),
            title: title.to_string(),
            status: ContextStatus::Active,
            created: now.clone(),
            updated: now,
            tags: Vec::new(),
            summary: String::new(),
            current_milestone: String::new(),
            next_steps: Vec::new(),
            related: Vec::new(),
            shareable: false,
        }
    }

    /// 生成 meta.json 内容
    pub fn to_meta_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// 生成 CONTEXT.md 内容
    pub fn to_context_md(&self) -> String {
        let mut md = format!("# {}\n\n", self.title);

        if !self.summary.is_empty() {
            md.push_str(&format!("## 背景\n{}\n\n", self.summary));
        }

        md.push_str(&format!("## 当前状态\n- 状态: {}\n", self.status));
        if !self.current_milestone.is_empty() {
            md.push_str(&format!("- 当前里程碑: {}\n", self.current_milestone));
        }
        md.push_str(&format!("- 创建: {}\n- 更新: {}\n\n", self.created, self.updated));

        if !self.tags.is_empty() {
            md.push_str(&format!("## 标签\n{}\n\n", self.tags.join(", ")));
        }

        if !self.next_steps.is_empty() {
            md.push_str("## 下一步\n");
            for step in &self.next_steps {
                md.push_str(&format!("- {}\n", step));
            }
            md.push('\n');
        }

        if !self.related.is_empty() {
            md.push_str("## 关联上下文\n");
            for r in &self.related {
                md.push_str(&format!("- {}\n", r));
            }
            md.push('\n');
        }

        md
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub context_id: String,
    pub date: String,
    pub entry_type: String,
    pub content: String,
    pub tags: Vec<String>,
}

impl Entry {
    pub fn new(context_id: &str, content: &str, entry_type: &str) -> Self {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string();
        Self {
            id: format!("{}_{}", context_id, chrono::Utc::now().timestamp()),
            context_id: context_id.to_string(),
            date: now,
            entry_type: entry_type.to_string(),
            content: content.to_string(),
            tags: Vec::new(),
        }
    }

    /// 生成 Markdown 文件内容（带 JSON frontmatter）
    pub fn to_markdown(&self) -> String {
        let frontmatter = serde_json::json!({
            "id": self.id,
            "date": self.date,
            "type": self.entry_type,
            "tags": self.tags,
        });

        format!(
            "---\n{}\n---\n\n## {}\n",
            serde_json::to_string_pretty(&frontmatter).unwrap_or_default(),
            self.content
        )
    }
}

/// 标题 → slug（只保留字母数字下划线连字符）
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_from_title() {
        let ctx = Context::from_title("Brand2Context — 品牌知识库");
        assert!(ctx.id.starts_with("ctx_"));
        assert_eq!(ctx.status, ContextStatus::Active);
    }

    #[test]
    fn test_entry_markdown() {
        let entry = Entry::new("ctx_test", "完成了某个功能", "progress");
        let md = entry.to_markdown();
        assert!(md.starts_with("---\n"));
        assert!(md.contains("完成了某个功能"));
    }

    #[test]
    fn test_status_parse() {
        let s: ContextStatus = "active".parse().unwrap();
        assert_eq!(s, ContextStatus::Active);
    }
}
