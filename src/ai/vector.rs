use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

/// 向量存储：在 SQLite 中存储 embeddings，支持余弦相似度搜索
pub struct VectorStore {
    conn: Connection,
}

impl VectorStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open vector db: {}", path.display()))?;
        conn.execute_batch("PRAGMA busy_timeout=5000;")?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA busy_timeout=5000;")?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS embeddings (
                id          TEXT PRIMARY KEY,
                context_id  TEXT NOT NULL,
                entry_id    TEXT,
                chunk_text  TEXT NOT NULL,
                embedding   TEXT NOT NULL,  -- JSON array of f32
                created     TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_emb_context ON embeddings(context_id);
            "#,
        )?;
        Ok(())
    }

    /// 存储一条 embedding
    pub fn upsert_embedding(
        &self,
        id: &str,
        context_id: &str,
        entry_id: Option<&str>,
        chunk_text: &str,
        embedding: &[f32],
    ) -> Result<()> {
        let emb_json = serde_json::to_string(embedding)?;
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"INSERT OR REPLACE INTO embeddings (id, context_id, entry_id, chunk_text, embedding, created)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![id, context_id, entry_id, chunk_text, emb_json, now],
        )?;
        Ok(())
    }

    /// 余弦相似度搜索
    pub fn search(&self, query_vec: &[f32], limit: usize) -> Result<Vec<SearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, context_id, entry_id, chunk_text, embedding FROM embeddings",
        )?;

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let context_id: String = row.get(1)?;
            let entry_id: Option<String> = row.get(2)?;
            let chunk_text: String = row.get(3)?;
            let emb_str: String = row.get(4)?;
            Ok((id, context_id, entry_id, chunk_text, emb_str))
        })?;

        let mut results: Vec<SearchResult> = Vec::new();
        for row in rows {
            let (id, context_id, entry_id, chunk_text, emb_str) = row?;
            let stored: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();
            let score = Self::cosine_similarity(query_vec, &stored);
            results.push(SearchResult {
                id,
                context_id,
                entry_id,
                chunk_text,
                score,
            });
        }

        // 按相似度降序排列
        results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    /// 在特定上下文内搜索
    pub fn search_in_context(
        &self,
        query_vec: &[f32],
        context_id: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, context_id, entry_id, chunk_text, embedding FROM embeddings WHERE context_id = ?1",
        )?;

        let rows = stmt.query_map(params![context_id], |row| {
            let id: String = row.get(0)?;
            let context_id: String = row.get(1)?;
            let entry_id: Option<String> = row.get(2)?;
            let chunk_text: String = row.get(3)?;
            let emb_str: String = row.get(4)?;
            Ok((id, context_id, entry_id, chunk_text, emb_str))
        })?;

        let mut results: Vec<SearchResult> = Vec::new();
        for row in rows {
            let (id, context_id, entry_id, chunk_text, emb_str) = row?;
            let stored: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();
            let score = Self::cosine_similarity(query_vec, &stored);
            results.push(SearchResult {
                id,
                context_id,
                entry_id,
                chunk_text,
                score,
            });
        }

        results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    /// 删除某上下文的所有 embedding
    pub fn delete_by_context(&self, context_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM embeddings WHERE context_id = ?1",
            params![context_id],
        )?;
        Ok(())
    }

    /// 检查某 entry 是否已索引
    pub fn has_entry(&self, entry_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM embeddings WHERE entry_id = ?1",
            params![entry_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// 删除某 entry 的所有 embedding
    pub fn delete_by_entry(&self, entry_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM embeddings WHERE entry_id = ?1",
            params![entry_id],
        )?;
        Ok(())
    }

    /// 获取总数
    pub fn count(&self) -> Result<i64> {
        let count: i64 = self.conn
            .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))?;
        Ok(count)
    }

    /// 余弦相似度
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub context_id: String,
    pub entry_id: Option<String>,
    pub chunk_text: String,
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_search() {
        let store = VectorStore::open_memory().unwrap();

        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let v3 = vec![0.9, 0.1, 0.0]; // 接近 v1

        store.upsert_embedding("e1", "ctx_a", Some("entry1"), "hello world", &v1).unwrap();
        store.upsert_embedding("e2", "ctx_a", Some("entry2"), "foo bar", &v2).unwrap();
        store.upsert_embedding("e3", "ctx_b", None, "hello rust", &v3).unwrap();

        // 搜索 [1,0,0] → v1 和 v3 应排前面
        let query = vec![1.0, 0.0, 0.0];
        let results = store.search(&query, 3).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results[0].score >= results[1].score);
        assert!(results[0].id == "e1" || results[0].id == "e3"); // 最相似
    }

    #[test]
    fn test_search_in_context() {
        let store = VectorStore::open_memory().unwrap();

        store.upsert_embedding("e1", "ctx_a", None, "text a", &[1.0, 0.0]).unwrap();
        store.upsert_embedding("e2", "ctx_b", None, "text b", &[0.9, 0.1]).unwrap();

        let results = store.search_in_context(&[1.0, 0.0], "ctx_a", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].context_id, "ctx_a");
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((VectorStore::cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((VectorStore::cosine_similarity(&a, &c) - 0.0).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((VectorStore::cosine_similarity(&a, &d) - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_has_entry_and_delete_by_entry() {
        let store = VectorStore::open_memory().unwrap();
        assert!(!store.has_entry("entry1").unwrap());
        store.upsert_embedding("e1", "ctx_a", Some("entry1"), "t", &[1.0]).unwrap();
        assert!(store.has_entry("entry1").unwrap());
        store.delete_by_entry("entry1").unwrap();
        assert!(!store.has_entry("entry1").unwrap());
    }

    #[test]
    fn test_delete_by_context() {
        let store = VectorStore::open_memory().unwrap();
        store.upsert_embedding("e1", "ctx_a", None, "t", &[1.0]).unwrap();
        store.upsert_embedding("e2", "ctx_b", None, "t", &[1.0]).unwrap();

        store.delete_by_context("ctx_a").unwrap();
        assert_eq!(store.count().unwrap(), 1);
    }
}
