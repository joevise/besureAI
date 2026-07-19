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
