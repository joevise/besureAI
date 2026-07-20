pub mod db;
pub mod vault;
pub mod models;

pub use vault::Vault;
pub use models::{Context, ContextStatus, Entry, EntryLink, EntryStatus, LinkRelation};
pub use db::{QueryFilter, Stats};

/// Check if current process has global vault access
pub fn can_access_all_vaults() -> bool {
    vault::Vault::can_access_all_vaults()
}

/// Vault 信息摘要（给 Dashboard / CLI 用）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VaultInfo {
    pub id: String,
    pub path: String,
    pub agent_name: String,
    pub agent_type: String,
    pub encrypted: bool,
    pub locked: bool,
    pub context_count: i64,
    pub entry_count: i64,
}
