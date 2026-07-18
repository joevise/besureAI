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

/// Entry status for closed-loop memory lifecycle.
/// active = live, superseded = replaced by newer, expired = past valid_until, archived = manually sidelined.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EntryStatus {
    Active,
    Superseded,
    Expired,
    Archived,
}

impl Default for EntryStatus {
    fn default() -> Self {
        EntryStatus::Active
    }
}

impl std::fmt::Display for EntryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryStatus::Active => write!(f, "active"),
            EntryStatus::Superseded => write!(f, "superseded"),
            EntryStatus::Expired => write!(f, "expired"),
            EntryStatus::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for EntryStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(EntryStatus::Active),
            "superseded" => Ok(EntryStatus::Superseded),
            "expired" => Ok(EntryStatus::Expired),
            "archived" => Ok(EntryStatus::Archived),
            _ => Err(format!("unknown entry status: {}", s)),
        }
    }
}

/// Link relation types for associative memory (8-dimension model).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LinkRelation {
    /// This entry was caused by the target
    CausedBy,
    /// This entry supersedes (replaces) the target
    Supersedes,
    /// General association
    RelatedTo,
    /// References a file path
    RefFile,
    /// References a git commit
    RefCommit,
    /// References a URL
    RefUrl,
}

impl Default for LinkRelation {
    fn default() -> Self {
        LinkRelation::RelatedTo
    }
}

impl std::fmt::Display for LinkRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkRelation::CausedBy => write!(f, "caused_by"),
            LinkRelation::Supersedes => write!(f, "supersedes"),
            LinkRelation::RelatedTo => write!(f, "related_to"),
            LinkRelation::RefFile => write!(f, "ref_file"),
            LinkRelation::RefCommit => write!(f, "ref_commit"),
            LinkRelation::RefUrl => write!(f, "ref_url"),
        }
    }
}

impl std::str::FromStr for LinkRelation {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "caused_by" | "causedby" => Ok(LinkRelation::CausedBy),
            "supersedes" | "supersede" => Ok(LinkRelation::Supersedes),
            "related_to" | "relatedto" | "related" => Ok(LinkRelation::RelatedTo),
            "ref_file" | "reffile" | "file" => Ok(LinkRelation::RefFile),
            "ref_commit" | "refcommit" | "commit" => Ok(LinkRelation::RefCommit),
            "ref_url" | "refurl" | "url" => Ok(LinkRelation::RefUrl),
            _ => Err(format!("unknown link relation: {}", s)),
        }
    }
}

/// An associative link between entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryLink {
    pub target_id: String,
    pub relation: LinkRelation,
}

/// A single memory record within a context.
///
/// entry_type can be: progress, milestone, decision, blocker, note, init,
/// config, lesson, question (free-form string, no enum constraint).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub context_id: String,
    pub date: String,
    pub entry_type: String,
    pub content: String,
    pub tags: Vec<String>,

    /// Associative links to other entries
    #[serde(default)]
    pub links: Vec<EntryLink>,

    /// When this entry becomes valid (default = date)
    #[serde(default)]
    pub valid_from: String,

    /// When this entry expires (None = forever)
    #[serde(default)]
    pub valid_until: Option<String>,

    /// Lifecycle status
    #[serde(default)]
    pub status: EntryStatus,

    /// If superseded, which entry replaced this one
    #[serde(default)]
    pub superseded_by: Option<String>,
}

impl Entry {
    pub fn new(context_id: &str, content: &str, entry_type: &str) -> Self {
        let now = chrono::Utc::now();
        let date_str = now.format("%Y-%m-%d %H:%M").to_string();
        let ts = now.timestamp_millis(); // millisecond precision for unique IDs
        Self {
            id: format!("{}_{}", context_id, ts),
            context_id: context_id.to_string(),
            date: date_str.clone(),
            entry_type: entry_type.to_string(),
            content: content.to_string(),
            tags: Vec::new(),
            links: Vec::new(),
            valid_from: date_str,
            valid_until: None,
            status: EntryStatus::Active,
            superseded_by: None,
        }
    }

    /// Generate Markdown file content (with JSON frontmatter)
    pub fn to_markdown(&self) -> String {
        let mut frontmatter = serde_json::json!({
            "id": self.id,
            "date": self.date,
            "type": self.entry_type,
            "tags": self.tags,
            "status": self.status.to_string(),
        });

        if !self.links.is_empty() {
            frontmatter["links"] = serde_json::json!(self.links);
        }
        if !self.valid_from.is_empty() {
            frontmatter["valid_from"] = serde_json::json!(self.valid_from);
        }
        if let Some(ref vu) = self.valid_until {
            frontmatter["valid_until"] = serde_json::json!(vu);
        }
        if let Some(ref sb) = self.superseded_by {
            frontmatter["superseded_by"] = serde_json::json!(sb);
        }

        format!(
            "---\n{}\n---\n\n## {}\n",
            serde_json::to_string_pretty(&frontmatter).unwrap_or_default(),
            self.content
        )
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
    /// Generate context id from title (slugify)
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

    /// Generate meta.json content
    pub fn to_meta_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Generate CONTEXT.md content
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

/// Title -> slug (keep only alphanumeric, underscore, hyphen)
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
        assert!(md.contains("status"));
    }

    #[test]
    fn test_status_parse() {
        let s: ContextStatus = "active".parse().unwrap();
        assert_eq!(s, ContextStatus::Active);
    }

    #[test]
    fn test_entry_status_parse() {
        let s: EntryStatus = "active".parse().unwrap();
        assert_eq!(s, EntryStatus::Active);
        let s2: EntryStatus = "superseded".parse().unwrap();
        assert_eq!(s2, EntryStatus::Superseded);
    }

    #[test]
    fn test_link_relation_parse() {
        let r: LinkRelation = "related_to".parse().unwrap();
        assert_eq!(r, LinkRelation::RelatedTo);
        let r2: LinkRelation = "caused_by".parse().unwrap();
        assert_eq!(r2, LinkRelation::CausedBy);
        let r3: LinkRelation = "file".parse().unwrap();
        assert_eq!(r3, LinkRelation::RefFile);
    }

    #[test]
    fn test_entry_defaults() {
        let entry = Entry::new("ctx_x", "test", "note");
        assert_eq!(entry.status, EntryStatus::Active);
        assert!(entry.links.is_empty());
        assert!(entry.valid_until.is_none());
        assert!(entry.superseded_by.is_none());
        assert!(!entry.valid_from.is_empty());
    }

    #[test]
    fn test_entry_serde_backward_compat() {
        // Old JSON without new fields should still deserialize
        let old_json = r#"{
            "id": "ctx_test_123",
            "context_id": "ctx_test",
            "date": "2026-01-01 10:00",
            "entry_type": "progress",
            "content": "test content",
            "tags": ["foo"]
        }"#;
        let entry: Entry = serde_json::from_str(old_json).unwrap();
        assert_eq!(entry.content, "test content");
        assert_eq!(entry.status, EntryStatus::Active);
        assert!(entry.links.is_empty());
    }
}
