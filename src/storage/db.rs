use anyhow::{Context as AnyhowContext, Result};
use rusqlite::{params, Connection};
use std::path::Path;

use super::models::{Context, ContextStatus, Entry};

/// SQLite 数据库管理
pub struct Database {
    conn: Connection,
}

impl Database {
    /// 打开/创建数据库
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open database: {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// 从内存创建（测试用）
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
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
                FOREIGN KEY (context_id) REFERENCES contexts(id)
            );

            CREATE INDEX IF NOT EXISTS idx_entries_context ON entries(context_id);
            CREATE INDEX IF NOT EXISTS idx_entries_date ON entries(date);
            CREATE INDEX IF NOT EXISTS idx_contexts_status ON contexts(status);
            "#,
        )?;
        Ok(())
    }

    /// 插入/更新上下文
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

    /// 获取上下文
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

    /// 列出所有上下文
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

    /// 按状态过滤
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

    /// 模糊匹配上下文 ID
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

    /// 更新上下文状态
    pub fn update_context_status(&self, id: &str, status: &ContextStatus) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.conn.execute(
            "UPDATE contexts SET status = ?1, updated = ?2 WHERE id = ?3",
            params![status.to_string(), now, id],
        )?;
        Ok(())
    }

    /// 更新上下文的 updated 时间戳
    pub fn touch_context(&self, id: &str) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.conn.execute(
            "UPDATE contexts SET updated = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    /// 添加进展记录
    pub fn add_entry(&self, entry: &Entry) -> Result<()> {
        self.conn.execute(
            r#"INSERT OR REPLACE INTO entries
               (id, context_id, date, entry_type, content, tags)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![
                entry.id,
                entry.context_id,
                entry.date,
                entry.entry_type,
                entry.content,
                serde_json::to_string(&entry.tags)?,
            ],
        )?;
        self.touch_context(&entry.context_id)?;
        Ok(())
    }

    /// 获取某上下文的所有记录（按时间倒序）
    pub fn list_entries(&self, context_id: &str) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, context_id, date, entry_type, content, tags FROM entries WHERE context_id = ?1 ORDER BY date DESC",
        )?;

        let entries = stmt
            .query_map(params![context_id], |row| {
                let tags_str: String = row.get(5)?;
                let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                Ok(Entry {
                    id: row.get(0)?,
                    context_id: row.get(1)?,
                    date: row.get(2)?,
                    entry_type: row.get(3)?,
                    content: row.get(4)?,
                    tags,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    /// 全文搜索（LIKE 匹配 content + title）
    pub fn search(&self, query: &str) -> Result<Vec<(Context, Entry)>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            r#"SELECT c.id, c.title, c.status, c.created, c.updated, c.tags, c.summary,
                      c.current_milestone, c.next_steps, c.related, c.shareable,
                      e.id, e.context_id, e.date, e.entry_type, e.content, e.tags
               FROM entries e
               JOIN contexts c ON e.context_id = c.id
               WHERE e.content LIKE ?1 OR c.title LIKE ?1 OR c.summary LIKE ?1
               ORDER BY e.date DESC"#,
        )?;

        let results = stmt
            .query_map(params![pattern], |row| {
                let ctx = Database::row_to_context_from_row(row, 0)?;
                let tags_str: String = row.get(16)?;
                let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                let entry = Entry {
                    id: row.get(11)?,
                    context_id: row.get(12)?,
                    date: row.get(13)?,
                    entry_type: row.get(14)?,
                    content: row.get(15)?,
                    tags,
                };
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

    /// 获取上下文数量
    pub fn count_contexts(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM contexts", [], |row| row.get(0))?;
        Ok(count)
    }

    /// 获取记录数量
    pub fn count_entries(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?;
        Ok(count)
    }
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
}
