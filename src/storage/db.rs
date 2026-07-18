use anyhow::{Context as AnyhowContext, Result};
use rusqlite::{params, Connection};
use std::path::Path;

use super::models::{Context, ContextStatus, Entry, EntryLink, EntryStatus, LinkRelation};

/// SQLite database manager
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open/create database
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open database: {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        let db = Self { conn };
        db.init_schema()?;
        db.run_migrations()?;
        Ok(db)
    }

    /// Create in-memory database (for tests)
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        db.run_migrations()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS contexts (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL,
                status      TEXT NOT NULL DEFAULT 'active',
                created     TEXT NOT NULL,
                updated     TEXT NOT NULL,
                tags        TEXT NOT NULL DEFAULT '[]',
                summary     TEXT NOT NULL DEFAULT '',
                current_milestone TEXT NOT NULL DEFAULT '',
                next_steps  TEXT NOT NULL DEFAULT '[]',
                related     TEXT NOT NULL DEFAULT '[]',
                shareable   INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS entries (
                id          TEXT PRIMARY KEY,
                context_id  TEXT NOT NULL,
                date        TEXT NOT NULL,
                entry_type  TEXT NOT NULL DEFAULT 'progress',
                content     TEXT NOT NULL,
                tags        TEXT NOT NULL DEFAULT '[]',
                links       TEXT NOT NULL DEFAULT '[]',
                valid_from  TEXT NOT NULL DEFAULT '',
                valid_until TEXT,
                status      TEXT NOT NULL DEFAULT 'active',
                superseded_by TEXT,
                resolved    BOOLEAN NOT NULL DEFAULT 0,
                FOREIGN KEY (context_id) REFERENCES contexts(id)
            );

            CREATE INDEX IF NOT EXISTS idx_entries_context ON entries(context_id);
            CREATE INDEX IF NOT EXISTS idx_entries_date ON entries(date);
            CREATE INDEX IF NOT EXISTS idx_contexts_status ON contexts(status);
            "#,
        )?;
        Ok(())
    }

    /// Idempotent migration: add new columns to legacy databases.
    /// Safe to run multiple times — ignores "duplicate column" errors.
    fn run_migrations(&self) -> Result<()> {
        let columns: &[(&str, &str)] = &[
            ("links", "TEXT NOT NULL DEFAULT '[]'"),
            ("valid_from", "TEXT NOT NULL DEFAULT ''"),
            ("valid_until", "TEXT"),
            ("status", "TEXT NOT NULL DEFAULT 'active'"),
            ("superseded_by", "TEXT"),
            ("resolved", "BOOLEAN NOT NULL DEFAULT 0"),
        ];

        for (col, def) in columns {
            let sql = format!("ALTER TABLE entries ADD COLUMN {} {}", col, def);
            match self.conn.execute(&sql, []) {
                Ok(_) => {}
                Err(e) => {
                    let msg = e.to_string();
                    if !msg.contains("duplicate column") {
                        return Err(anyhow::anyhow!("migration error: {}", e));
                    }
                }
            }
        }

        // Add status index if not exists
        let _ = self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_entries_status ON entries(status)",
            [],
        );

        Ok(())
    }

    /// Insert/update context
    pub fn upsert_context(&self, ctx: &Context) -> Result<()> {
        self.conn.execute(
            r#"INSERT OR REPLACE INTO contexts
               (id, title, status, created, updated, tags, summary, current_milestone, next_steps, related, shareable)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                ctx.id,
                ctx.title,
                ctx.status.to_string(),
                ctx.created,
                ctx.updated,
                serde_json::to_string(&ctx.tags)?,
                ctx.summary,
                ctx.current_milestone,
                serde_json::to_string(&ctx.next_steps)?,
                serde_json::to_string(&ctx.related)?,
                ctx.shareable as i32,
            ],
        )?;
        Ok(())
    }

    /// Get context by id
    pub fn get_context(&self, id: &str) -> Result<Option<Context>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, status, created, updated, tags, summary, current_milestone, next_steps, related, shareable FROM contexts WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(self.row_to_context(row)?))
        } else {
            Ok(None)
        }
    }

    /// List all contexts
    pub fn list_contexts(&self) -> Result<Vec<Context>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, status, created, updated, tags, summary, current_milestone, next_steps, related, shareable FROM contexts ORDER BY updated DESC",
        )?;

        let contexts = stmt
            .query_map([], |row| self.row_to_context(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(contexts)
    }

    /// Filter contexts by status
    pub fn list_contexts_by_status(&self, status: &ContextStatus) -> Result<Vec<Context>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, status, created, updated, tags, summary, current_milestone, next_steps, related, shareable FROM contexts WHERE status = ?1 ORDER BY updated DESC",
        )?;

        let contexts = stmt
            .query_map(params![status.to_string()], |row| self.row_to_context(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(contexts)
    }

    /// Fuzzy match context id/title
    pub fn find_contexts_fuzzy(&self, query: &str) -> Result<Vec<Context>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, title, status, created, updated, tags, summary, current_milestone, next_steps, related, shareable FROM contexts WHERE id LIKE ?1 OR title LIKE ?1 ORDER BY updated DESC",
        )?;

        let contexts = stmt
            .query_map(params![pattern], |row| self.row_to_context(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(contexts)
    }

    /// Update context status
    pub fn update_context_status(&self, id: &str, status: &ContextStatus) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.conn.execute(
            "UPDATE contexts SET status = ?1, updated = ?2 WHERE id = ?3",
            params![status.to_string(), now, id],
        )?;
        Ok(())
    }

    /// Touch context's updated timestamp
    pub fn touch_context(&self, id: &str) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.conn.execute(
            "UPDATE contexts SET updated = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    /// Add an entry
    pub fn add_entry(&self, entry: &Entry) -> Result<()> {
        self.conn.execute(
            r#"INSERT OR REPLACE INTO entries
               (id, context_id, date, entry_type, content, tags, links, valid_from, valid_until, status, superseded_by, resolved)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
            params![
                entry.id,
                entry.context_id,
                entry.date,
                entry.entry_type,
                entry.content,
                serde_json::to_string(&entry.tags)?,
                serde_json::to_string(&entry.links)?,
                entry.valid_from,
                entry.valid_until,
                entry.status.to_string(),
                entry.superseded_by,
                entry.resolved,
            ],
        )?;
        self.touch_context(&entry.context_id)?;
        Ok(())
    }

    /// Get a single entry by id
    pub fn get_entry(&self, id: &str) -> Result<Option<Entry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, context_id, date, entry_type, content, tags, links, valid_from, valid_until, status, superseded_by, resolved FROM entries WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(self.row_to_entry(row)?))
        } else {
            Ok(None)
        }
    }

    /// List all entries for a context (newest first)
    pub fn list_entries(&self, context_id: &str) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, context_id, date, entry_type, content, tags, links, valid_from, valid_until, status, superseded_by, resolved FROM entries WHERE context_id = ?1 ORDER BY date DESC",
        )?;

        let entries = stmt
            .query_map(params![context_id], |row| self.row_to_entry(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    /// List entries by status within a context
    pub fn list_entries_by_status(&self, context_id: &str, status: &EntryStatus) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, context_id, date, entry_type, content, tags, links, valid_from, valid_until, status, superseded_by, resolved FROM entries WHERE context_id = ?1 AND status = ?2 ORDER BY date DESC",
        )?;

        let entries = stmt
            .query_map(params![context_id, status.to_string()], |row| self.row_to_entry(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    /// List entries that expire before a given date
    pub fn list_expiring_entries(&self, context_id: &str, before_date: &str) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, context_id, date, entry_type, content, tags, links, valid_from, valid_until, status, superseded_by, resolved FROM entries WHERE context_id = ?1 AND status = 'active' AND valid_until IS NOT NULL AND valid_until < ?2 ORDER BY valid_until ASC",
        )?;

        let entries = stmt
            .query_map(params![context_id, before_date], |row| self.row_to_entry(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    /// Update an entry's status (and optionally set superseded_by)
    pub fn update_entry_status(&self, id: &str, status: &EntryStatus, superseded_by: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE entries SET status = ?1, superseded_by = ?2 WHERE id = ?3",
            params![status.to_string(), superseded_by, id],
        )?;
        Ok(())
    }

    /// Add a link to an entry (appends to links JSON array)
    pub fn add_entry_link(&self, entry_id: &str, link: &EntryLink) -> Result<()> {
        // Fetch current links
        let entry = self.get_entry(entry_id)?
            .context("entry not found for linking")?;

        let mut links = entry.links;
        // Avoid duplicate (same target + same relation)
        let exists = links.iter().any(|l| l.target_id == link.target_id && l.relation == link.relation);
        if !exists {
            links.push(link.clone());
        }

        let links_json = serde_json::to_string(&links)?;
        self.conn.execute(
            "UPDATE entries SET links = ?1 WHERE id = ?2",
            params![links_json, entry_id],
        )?;
        Ok(())
    }

    /// Full-text search (LIKE match on content + title)
    pub fn search(&self, query: &str) -> Result<Vec<(Context, Entry)>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            r#"SELECT c.id, c.title, c.status, c.created, c.updated, c.tags, c.summary,
                      c.current_milestone, c.next_steps, c.related, c.shareable,
                      e.id, e.context_id, e.date, e.entry_type, e.content, e.tags,
                      e.links, e.valid_from, e.valid_until, e.status, e.superseded_by, e.resolved
               FROM entries e
               JOIN contexts c ON e.context_id = c.id
               WHERE e.content LIKE ?1 OR c.title LIKE ?1 OR c.summary LIKE ?1
               ORDER BY e.date DESC"#,
        )?;

        let results = stmt
            .query_map(params![pattern], |row| {
                let ctx = Database::row_to_context_from_row(row, 0)?;
                let entry = Database::row_to_entry_from_row(row, 11)?;
                Ok((ctx, entry))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    fn row_to_context(&self, row: &rusqlite::Row) -> Result<Context, rusqlite::Error> {
        Database::row_to_context_from_row(row, 0)
    }

    fn row_to_context_from_row(
        row: &rusqlite::Row,
        offset: usize,
    ) -> Result<Context, rusqlite::Error> {
        let tags_str: String = row.get(offset + 5)?;
        let next_str: String = row.get(offset + 8)?;
        let related_str: String = row.get(offset + 9)?;
        let status_str: String = row.get(offset + 2)?;

        Ok(Context {
            id: row.get(offset)?,
            title: row.get(offset + 1)?,
            status: status_str.parse().unwrap_or(ContextStatus::Active),
            created: row.get(offset + 3)?,
            updated: row.get(offset + 4)?,
            tags: serde_json::from_str(&tags_str).unwrap_or_default(),
            summary: row.get(offset + 6)?,
            current_milestone: row.get(offset + 7)?,
            next_steps: serde_json::from_str(&next_str).unwrap_or_default(),
            related: serde_json::from_str(&related_str).unwrap_or_default(),
            shareable: row.get(offset + 10)?,
        })
    }

    fn row_to_entry(&self, row: &rusqlite::Row) -> Result<Entry, rusqlite::Error> {
        Database::row_to_entry_from_row(row, 0)
    }

    fn row_to_entry_from_row(
        row: &rusqlite::Row,
        offset: usize,
    ) -> Result<Entry, rusqlite::Error> {
        let tags_str: String = row.get(offset + 5)?;
        let links_str: String = row.get(offset + 6).unwrap_or_else(|_| "[]".to_string());
        let status_str: String = row.get(offset + 9).unwrap_or_else(|_| "active".to_string());

        Ok(Entry {
            id: row.get(offset)?,
            context_id: row.get(offset + 1)?,
            date: row.get(offset + 2)?,
            entry_type: row.get(offset + 3)?,
            content: row.get(offset + 4)?,
            tags: serde_json::from_str(&tags_str).unwrap_or_default(),
            links: serde_json::from_str(&links_str).unwrap_or_default(),
            valid_from: row.get(offset + 7).unwrap_or_default(),
            valid_until: row.get(offset + 8).ok(),
            status: status_str.parse().unwrap_or(EntryStatus::Active),
            superseded_by: row.get(offset + 10).ok(),
            resolved: row.get::<_, bool>(offset + 11).unwrap_or(false),
        })
    }

    /// Count contexts
    pub fn count_contexts(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM contexts", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Count entries
    pub fn count_entries(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Update an entry's resolved flag
    pub fn update_entry_resolved(&self, id: &str, resolved: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE entries SET resolved = ?1 WHERE id = ?2",
            params![resolved, id],
        )?;
        Ok(())
    }

    /// Unified query over entries with filters
    pub fn query_entries(&self, filter: &QueryFilter) -> Result<Vec<Entry>> {
        let mut sql = String::from(
            "SELECT id, context_id, date, entry_type, content, tags, links, valid_from, valid_until, status, superseded_by, resolved FROM entries WHERE 1=1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if !filter.all_contexts {
            if let Some(ref cid) = filter.context_id {
                sql.push_str(&format!(" AND context_id = ?{}", param_values.len() + 1));
                param_values.push(Box::new(cid.clone()));
            }
        }
        if let Some(ref from) = filter.from_date {
            sql.push_str(&format!(" AND substr(date, 1, 10) >= ?{}", param_values.len() + 1));
            param_values.push(Box::new(from.clone()));
        }
        if let Some(ref to) = filter.to_date {
            sql.push_str(&format!(" AND substr(date, 1, 10) <= ?{}", param_values.len() + 1));
            param_values.push(Box::new(to.clone()));
        }
        if !filter.entry_types.is_empty() {
            let placeholders: Vec<String> = filter
                .entry_types
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", param_values.len() + 1 + i))
                .collect();
            sql.push_str(&format!(" AND entry_type IN ({})", placeholders.join(", ")));
            for t in &filter.entry_types {
                param_values.push(Box::new(t.clone()));
            }
        }
        if let Some(ref kw) = filter.keyword {
            sql.push_str(&format!(" AND content LIKE ?{}", param_values.len() + 1));
            param_values.push(Box::new(format!("%{}%", kw)));
        }
        if let Some(resolved) = filter.resolved {
            sql.push_str(&format!(" AND resolved = ?{}", param_values.len() + 1));
            param_values.push(Box::new(resolved));
        }

        sql.push_str(" ORDER BY date DESC");
        sql.push_str(&format!(" LIMIT ?{}", param_values.len() + 1));
        param_values.push(Box::new(filter.limit as i64));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let entries = stmt
            .query_map(params_refs.as_slice(), |row| {
                Database::row_to_entry_from_row(row, 0)
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    /// Append text to an entry's content (with timestamped separator)
    pub fn append_entry_content(&self, id: &str, text: &str) -> Result<()> {
        let entry = self
            .get_entry(id)?
            .context("entry not found for append")?;
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string();
        let new_content = format!(
            "{}\n\n---\n**[追加 {}]**\n{}",
            entry.content, now, text
        );
        self.conn.execute(
            "UPDATE entries SET content = ?1 WHERE id = ?2",
            params![new_content, id],
        )?;
        Ok(())
    }

    /// Aggregate statistics: by context / type / status / resolved / recent activity
    pub fn get_stats(&self) -> Result<Stats> {
        let total_contexts = self.count_contexts()?;
        let total_entries = self.count_entries()?;

        let mut stmt = self.conn.prepare(
            "SELECT c.title, COUNT(e.id) FROM contexts c LEFT JOIN entries e ON e.context_id = c.id GROUP BY c.id ORDER BY COUNT(e.id) DESC",
        )?;
        let by_context = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut stmt = self.conn.prepare(
            "SELECT entry_type, COUNT(*) FROM entries GROUP BY entry_type ORDER BY COUNT(*) DESC",
        )?;
        let by_type = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut stmt = self.conn.prepare(
            "SELECT status, COUNT(*) FROM entries GROUP BY status ORDER BY COUNT(*) DESC",
        )?;
        let by_status = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let resolved_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM entries WHERE resolved = 1",
            [],
            |row| row.get(0),
        )?;

        let seven_days_ago = (chrono::Utc::now() - chrono::Duration::days(7))
            .format("%Y-%m-%d")
            .to_string();
        let mut stmt = self.conn.prepare(
            "SELECT substr(date, 1, 10) AS d, COUNT(*) FROM entries WHERE substr(date, 1, 10) >= ?1 GROUP BY d ORDER BY d DESC",
        )?;
        let recent_activity = stmt
            .query_map(params![seven_days_ago], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(Stats {
            total_contexts,
            total_entries,
            by_context,
            by_type,
            by_status,
            resolved_count,
            recent_activity,
        })
    }
}

/// Filter for unified entry queries
#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    /// None = current context (resolved by caller)
    pub context_id: Option<String>,
    /// true = search across all contexts
    pub all_contexts: bool,
    /// YYYY-MM-DD
    pub from_date: Option<String>,
    /// YYYY-MM-DD
    pub to_date: Option<String>,
    /// empty = all types
    pub entry_types: Vec<String>,
    pub keyword: Option<String>,
    /// None = all, Some(true) = resolved only, Some(false) = unresolved only
    pub resolved: Option<bool>,
    pub limit: usize,
}

/// Aggregate statistics snapshot
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Stats {
    pub total_contexts: i64,
    pub total_entries: i64,
    pub by_context: Vec<(String, i64)>,
    pub by_type: Vec<(String, i64)>,
    pub by_status: Vec<(String, i64)>,
    pub resolved_count: i64,
    pub recent_activity: Vec<(String, i64)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_crud() {
        let db = Database::open_memory().unwrap();

        let ctx = Context::from_title("Test Project");
        db.upsert_context(&ctx).unwrap();

        let found = db.get_context(&ctx.id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Test Project");

        let ctxs = db.list_contexts().unwrap();
        assert_eq!(ctxs.len(), 1);
    }

    #[test]
    fn test_entries() {
        let db = Database::open_memory().unwrap();

        let ctx = Context::from_title("Entry Test");
        db.upsert_context(&ctx).unwrap();

        let entry = Entry::new(&ctx.id, "完成了第一版", "progress");
        db.add_entry(&entry).unwrap();

        let entries = db.list_entries(&ctx.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "完成了第一版");
        assert_eq!(entries[0].status, EntryStatus::Active);
        assert!(entries[0].links.is_empty());
    }

    #[test]
    fn test_search() {
        let db = Database::open_memory().unwrap();

        let ctx = Context::from_title("Brand Analysis");
        db.upsert_context(&ctx).unwrap();

        let entry = Entry::new(&ctx.id, "完成了品牌调研", "progress");
        db.add_entry(&entry).unwrap();

        let results = db.search("品牌").unwrap();
        assert!(!results.is_empty());

        let results2 = db.search("Brand").unwrap();
        assert!(!results2.is_empty());
    }

    #[test]
    fn test_fuzzy_match() {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("Quant Report");
        db.upsert_context(&ctx).unwrap();

        let found = db.find_contexts_fuzzy("quant").unwrap();
        assert!(!found.is_empty());
    }

    #[test]
    fn test_entry_status_update() {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("Status Test");
        db.upsert_context(&ctx).unwrap();

        let entry = Entry::new(&ctx.id, "test content", "note");
        db.add_entry(&entry).unwrap();

        db.update_entry_status(&entry.id, &EntryStatus::Expired, None).unwrap();

        let fetched = db.get_entry(&entry.id).unwrap().unwrap();
        assert_eq!(fetched.status, EntryStatus::Expired);
    }

    #[test]
    fn test_entry_link() {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("Link Test");
        db.upsert_context(&ctx).unwrap();

        let entry1 = Entry::new(&ctx.id, "first entry", "progress");
        let entry2 = Entry::new(&ctx.id, "second entry", "progress");
        db.add_entry(&entry1).unwrap();
        db.add_entry(&entry2).unwrap();

        let link = EntryLink {
            target_id: entry2.id.clone(),
            relation: LinkRelation::RelatedTo,
        };
        db.add_entry_link(&entry1.id, &link).unwrap();

        let fetched = db.get_entry(&entry1.id).unwrap().unwrap();
        assert_eq!(fetched.links.len(), 1);
        assert_eq!(fetched.links[0].target_id, entry2.id);
    }

    #[test]
    fn test_list_by_status() {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("ListStatus Test");
        db.upsert_context(&ctx).unwrap();

        let mut e1 = Entry::new(&ctx.id, "active one", "note");
        e1.id = format!("{}_{}_1", ctx.id, chrono::Utc::now().timestamp());
        let mut e2 = Entry::new(&ctx.id, "archived one", "note");
        e2.id = format!("{}_{}_2", ctx.id, chrono::Utc::now().timestamp());
        db.add_entry(&e1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        db.add_entry(&e2).unwrap();
        db.update_entry_status(&e2.id, &EntryStatus::Archived, None).unwrap();

        let active = db.list_entries_by_status(&ctx.id, &EntryStatus::Active).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].content, "active one");

        let archived = db.list_entries_by_status(&ctx.id, &EntryStatus::Archived).unwrap();
        assert_eq!(archived.len(), 1);
    }

    #[test]
    fn test_migration_idempotent() {
        // Running migrations multiple times should not error
        let db = Database::open_memory().unwrap();
        // Migration already ran in open_memory, run again
        db.run_migrations().unwrap();
        db.run_migrations().unwrap();
    }

    fn make_entry(db: &Database, ctx_id: &str, content: &str, entry_type: &str, date: &str) -> Entry {
        let mut e = Entry::new(ctx_id, content, entry_type);
        e.id = format!("{}_{}_{}", ctx_id, entry_type, db.count_entries().unwrap());
        e.date = date.to_string();
        db.add_entry(&e).unwrap();
        e
    }

    #[test]
    fn test_query_by_date_range() {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("Query Date Test");
        db.upsert_context(&ctx).unwrap();

        make_entry(&db, &ctx.id, "early entry", "progress", "2026-07-01 10:00");
        make_entry(&db, &ctx.id, "mid entry", "progress", "2026-07-10 10:00");
        make_entry(&db, &ctx.id, "late entry", "progress", "2026-07-18 10:00");

        let filter = QueryFilter {
            context_id: Some(ctx.id.clone()),
            from_date: Some("2026-07-05".to_string()),
            to_date: Some("2026-07-15".to_string()),
            limit: 20,
            ..Default::default()
        };
        let results = db.query_entries(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "mid entry");

        let filter2 = QueryFilter {
            context_id: Some(ctx.id.clone()),
            from_date: Some("2026-07-01".to_string()),
            limit: 20,
            ..Default::default()
        };
        assert_eq!(db.query_entries(&filter2).unwrap().len(), 3);
    }

    #[test]
    fn test_query_by_type() {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("Query Type Test");
        db.upsert_context(&ctx).unwrap();

        make_entry(&db, &ctx.id, "a progress", "progress", "2026-07-10 10:00");
        make_entry(&db, &ctx.id, "a milestone", "milestone", "2026-07-11 10:00");
        make_entry(&db, &ctx.id, "a lesson", "lesson", "2026-07-12 10:00");

        let filter = QueryFilter {
            context_id: Some(ctx.id.clone()),
            entry_types: vec!["milestone".to_string(), "lesson".to_string()],
            limit: 20,
            ..Default::default()
        };
        let results = db.query_entries(&filter).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.entry_type == "milestone" || e.entry_type == "lesson"));
    }

    #[test]
    fn test_query_by_keyword() {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("Query Keyword Test");
        db.upsert_context(&ctx).unwrap();

        make_entry(&db, &ctx.id, "Besure V3 shipped", "milestone", "2026-07-10 10:00");
        make_entry(&db, &ctx.id, "unrelated note", "note", "2026-07-11 10:00");

        let filter = QueryFilter {
            context_id: Some(ctx.id.clone()),
            keyword: Some("V3".to_string()),
            limit: 20,
            ..Default::default()
        };
        let results = db.query_entries(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Besure V3 shipped");
    }

    #[test]
    fn test_query_resolved_filter() {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("Query Resolved Test");
        db.upsert_context(&ctx).unwrap();

        let e1 = make_entry(&db, &ctx.id, "resolved one", "note", "2026-07-10 10:00");
        make_entry(&db, &ctx.id, "open one", "note", "2026-07-11 10:00");
        db.update_entry_resolved(&e1.id, true).unwrap();

        let resolved_only = db.query_entries(&QueryFilter {
            context_id: Some(ctx.id.clone()),
            resolved: Some(true),
            limit: 20,
            ..Default::default()
        }).unwrap();
        assert_eq!(resolved_only.len(), 1);
        assert_eq!(resolved_only[0].content, "resolved one");
        assert!(resolved_only[0].resolved);

        let unresolved_only = db.query_entries(&QueryFilter {
            context_id: Some(ctx.id.clone()),
            resolved: Some(false),
            limit: 20,
            ..Default::default()
        }).unwrap();
        assert_eq!(unresolved_only.len(), 1);
        assert_eq!(unresolved_only[0].content, "open one");
        assert!(!unresolved_only[0].resolved);
    }

    #[test]
    fn test_resolve_entry() {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("Resolve Test");
        db.upsert_context(&ctx).unwrap();

        let e = make_entry(&db, &ctx.id, "to resolve", "blocker", "2026-07-10 10:00");
        assert!(!e.resolved);

        db.update_entry_resolved(&e.id, true).unwrap();
        let fetched = db.get_entry(&e.id).unwrap().unwrap();
        assert!(fetched.resolved);

        db.update_entry_resolved(&e.id, false).unwrap();
        let fetched = db.get_entry(&e.id).unwrap().unwrap();
        assert!(!fetched.resolved);
    }

    #[test]
    fn test_append_entry() {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("Append Test");
        db.upsert_context(&ctx).unwrap();

        let e = make_entry(&db, &ctx.id, "original content", "note", "2026-07-10 10:00");
        db.append_entry_content(&e.id, "补充内容").unwrap();

        let fetched = db.get_entry(&e.id).unwrap().unwrap();
        assert!(fetched.content.starts_with("original content"));
        assert!(fetched.content.contains("---"));
        assert!(fetched.content.contains("**[追加 "));
        assert!(fetched.content.contains("补充内容"));
    }

    #[test]
    fn test_stats() {
        let db = Database::open_memory().unwrap();
        let ctx1 = Context::from_title("Stats A");
        let ctx2 = Context::from_title("Stats B");
        db.upsert_context(&ctx1).unwrap();
        db.upsert_context(&ctx2).unwrap();

        let today = chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string();
        let e1 = make_entry(&db, &ctx1.id, "a1", "progress", &today);
        make_entry(&db, &ctx1.id, "a2", "milestone", &today);
        make_entry(&db, &ctx2.id, "b1", "progress", &today);
        db.update_entry_resolved(&e1.id, true).unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_contexts, 2);
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.resolved_count, 1);
        assert_eq!(stats.by_context.len(), 2);
        assert_eq!(stats.by_context[0].1, 2); // ctx1 has most entries
        assert!(stats.by_type.iter().any(|(t, c)| t == "progress" && *c == 2));
        assert!(stats.by_type.iter().any(|(t, c)| t == "milestone" && *c == 1));
        assert!(stats.by_status.iter().any(|(s, c)| s == "active" && *c == 3));
        assert_eq!(stats.recent_activity.len(), 1);
        assert_eq!(stats.recent_activity[0].1, 3);
    }

    #[test]
    fn test_query_all_contexts() {
        let db = Database::open_memory().unwrap();
        let ctx1 = Context::from_title("All A");
        let ctx2 = Context::from_title("All B");
        db.upsert_context(&ctx1).unwrap();
        db.upsert_context(&ctx2).unwrap();

        make_entry(&db, &ctx1.id, "in a", "note", "2026-07-10 10:00");
        make_entry(&db, &ctx2.id, "in b", "note", "2026-07-11 10:00");

        let results = db.query_entries(&QueryFilter {
            all_contexts: true,
            limit: 20,
            ..Default::default()
        }).unwrap();
        assert_eq!(results.len(), 2);

        let scoped = db.query_entries(&QueryFilter {
            context_id: Some(ctx1.id.clone()),
            limit: 20,
            ..Default::default()
        }).unwrap();
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].content, "in a");
    }
}
