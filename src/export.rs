//! .besure encrypted export/import format
//!
//! File layout:
//!   MAGIC:    "BESURE1" (7 bytes)
//!   VERSION:  0x01      (1 byte)
//!   SALT:     16 bytes  (random, Argon2id)
//!   NONCE:    12 bytes  (random, AES-GCM)
//!   CIPHERTEXT: rest    (AES-256-GCM of JSON payload)

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{bail, Context as AnyhowContext, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::storage::db::Database;
use crate::storage::models::{Context, Entry};

const MAGIC: &[u8] = b"BESURE1";
const FORMAT_VERSION: u8 = 0x01;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

// Same Argon2id parameters as VaultCrypto
const ARGON2_MEMORY: u32 = 65536;
const ARGON2_TIME: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;

/// Result of an import operation
#[derive(Debug)]
pub struct ImportResult {
    pub context: Context,
    pub entries_imported: usize,
    pub entries_skipped: usize,
}

#[derive(Serialize, Deserialize)]
struct ExportPayload {
    version: u32,
    context: Context,
    entries: Vec<Entry>,
}

fn derive_key(password: &str, salt: &[u8]) -> [u8; KEY_LEN] {
    let params = Params::new(ARGON2_MEMORY, ARGON2_PARALLELISM, ARGON2_TIME, Some(KEY_LEN))
        .expect("valid argon2 params");
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .expect("argon2 derive failed");
    key
}

/// Build the encrypted .besure file bytes for a context.
/// Returns (file_bytes, entry_count).
pub fn export_bytes(db: &Database, context_id: &str, password: &str) -> Result<(Vec<u8>, usize)> {
    let ctx = db
        .get_context(context_id)?
        .with_context(|| format!("context '{}' not found", context_id))?;
    let entries = db.list_entries(context_id)?;

    let payload = ExportPayload {
        version: 1,
        context: ctx,
        entries,
    };
    let entry_count = payload.entries.len();
    let json = serde_json::to_vec(&payload)?;

    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = derive_key(password, &salt);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), json.as_slice())
        .map_err(|e| anyhow::anyhow!("AES-GCM encryption failed: {:?}", e))?;

    let mut out = Vec::with_capacity(MAGIC.len() + 1 + SALT_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok((out, entry_count))
}

/// Export a context to an encrypted .besure file. Returns entry count.
pub fn export_encrypted(
    db: &Database,
    context_id: &str,
    password: &str,
    output_path: &Path,
) -> Result<usize> {
    let (bytes, count) = export_bytes(db, context_id, password)?;
    std::fs::write(output_path, &bytes)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    Ok(count)
}

/// Decrypt .besure bytes and import into the database.
/// Entries whose id already exists are skipped.
/// If `target_context_id` is Some, all entries are imported into that existing
/// context instead of restoring the original context from the payload.
pub fn import_bytes(db: &Database, data: &[u8], password: &str, target_context_id: Option<&str>) -> Result<ImportResult> {
    let header_len = MAGIC.len() + 1 + SALT_LEN + NONCE_LEN;
    if data.len() < header_len + 16 {
        bail!("file too short to be a .besure export");
    }
    if &data[..MAGIC.len()] != MAGIC {
        bail!("not a .besure file (bad magic)");
    }
    let version = data[MAGIC.len()];
    if version != FORMAT_VERSION {
        bail!("unsupported .besure format version: {}", version);
    }
    let salt = &data[MAGIC.len() + 1..MAGIC.len() + 1 + SALT_LEN];
    let nonce_bytes = &data[MAGIC.len() + 1 + SALT_LEN..header_len];
    let ciphertext = &data[header_len..];

    let key = derive_key(password, salt);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| anyhow::anyhow!("decryption failed: wrong password or corrupted file"))?;

    let payload: ExportPayload = serde_json::from_slice(&plaintext)
        .context("decrypted payload is not valid .besure JSON")?;

    let context = match target_context_id {
        Some(target_id) => {
            let ctx = db
                .get_context(target_id)?
                .with_context(|| format!("target context '{}' not found", target_id))?;
            ctx
        }
        None => {
            // Insert context only if it doesn't already exist
            if db.get_context(&payload.context.id)?.is_none() {
                db.upsert_context(&payload.context)?;
            }
            payload.context
        }
    };

    let mut imported = 0usize;
    let mut skipped = 0usize;
    for entry in &payload.entries {
        if db.get_entry(&entry.id)?.is_some() {
            skipped += 1;
        } else {
            let mut entry = entry.clone();
            entry.context_id = context.id.clone();
            db.add_entry(&entry)?;
            imported += 1;
        }
    }

    Ok(ImportResult {
        context,
        entries_imported: imported,
        entries_skipped: skipped,
    })
}

/// Import a .besure file into the current vault.
pub fn import_encrypted(db: &Database, file_path: &Path, password: &str, target_context_id: Option<&str>) -> Result<ImportResult> {
    let data = std::fs::read(file_path)
        .with_context(|| format!("failed to read {}", file_path.display()))?;
    import_bytes(db, &data, password, target_context_id)
}

// === Minimal base64 (no new dependencies) ===

const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_CHARS[(n >> 18) as usize & 63] as char);
        out.push(B64_CHARS[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64_CHARS[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64_CHARS[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn b64_decode(s: &str) -> Result<Vec<u8>> {
    let mut table = [255u8; 256];
    for (i, &c) in B64_CHARS.iter().enumerate() {
        table[c as usize] = i as u8;
    }
    let cleaned: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if cleaned.len() % 4 != 0 {
        bail!("invalid base64 length");
    }
    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    for chunk in cleaned.chunks(4) {
        let pad = chunk.iter().filter(|&&b| b == b'=').count();
        if pad > 2 || (pad > 0 && chunk[3] != b'=') {
            bail!("invalid base64 padding");
        }
        let mut n: u32 = 0;
        for (i, &b) in chunk.iter().enumerate() {
            let v = if b == b'=' { 0 } else { table[b as usize] };
            if b != b'=' && v == 255 {
                bail!("invalid base64 character");
            }
            n |= (v as u32) << (18 - i * 6);
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{EntryLink, EntryStatus, LinkRelation};

    fn setup_db() -> (Database, Context) {
        let db = Database::open_memory().unwrap();
        let ctx = Context::from_title("Export Test");
        db.upsert_context(&ctx).unwrap();
        (db, ctx)
    }

    #[test]
    fn test_export_import_roundtrip() {
        let (db, ctx) = setup_db();

        let mut e1 = Entry::new(&ctx.id, "first entry with 中文", "progress");
        e1.tags = vec!["后端开发".to_string()];
        e1.links = vec![EntryLink {
            target_id: "ctx_other_1".to_string(),
            relation: LinkRelation::RelatedTo,
        }];
        let mut e2 = Entry::new(&ctx.id, "second entry", "decision");
        e2.id = format!("{}_unique2", ctx.id);
        e2.status = EntryStatus::Expired;
        e2.resolved = true;
        e2.valid_until = Some("2026-12-31".to_string());
        db.add_entry(&e1).unwrap();
        db.add_entry(&e2).unwrap();

        let tmp = std::env::temp_dir().join(format!("besure_export_test_{}.besure", std::process::id()));
        let count = export_encrypted(&db, &ctx.id, "test123", &tmp).unwrap();
        assert_eq!(count, 2);

        // File starts with magic, rest is not plaintext JSON
        let raw = std::fs::read(&tmp).unwrap();
        assert_eq!(&raw[..7], b"BESURE1");
        assert_eq!(raw[7], 0x01);
        assert!(std::str::from_utf8(&raw).is_err() || !String::from_utf8_lossy(&raw).contains("first entry"));

        // Import into a fresh DB
        let db2 = Database::open_memory().unwrap();
        let result = import_encrypted(&db2, &tmp, "test123", None).unwrap();
        assert_eq!(result.context.id, ctx.id);
        assert_eq!(result.entries_imported, 2);
        assert_eq!(result.entries_skipped, 0);

        let restored = db2.get_entry(&e1.id).unwrap().unwrap();
        assert_eq!(restored.content, "first entry with 中文");
        assert_eq!(restored.tags, vec!["后端开发".to_string()]);
        assert_eq!(restored.links.len(), 1);
        assert_eq!(restored.links[0].target_id, "ctx_other_1");

        let restored2 = db2.get_entry(&e2.id).unwrap().unwrap();
        assert_eq!(restored2.status, EntryStatus::Expired);
        assert!(restored2.resolved);
        assert_eq!(restored2.valid_until.as_deref(), Some("2026-12-31"));

        // Re-import: all entries skipped
        let result2 = import_encrypted(&db2, &tmp, "test123", None).unwrap();
        assert_eq!(result2.entries_imported, 0);
        assert_eq!(result2.entries_skipped, 2);

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_import_into_target_context() {
        let (db, ctx) = setup_db();
        let mut e1 = Entry::new(&ctx.id, "entry one", "progress");
        e1.id = format!("{}_t1", ctx.id);
        let mut e2 = Entry::new(&ctx.id, "entry two", "note");
        e2.id = format!("{}_t2", ctx.id);
        db.add_entry(&e1).unwrap();
        db.add_entry(&e2).unwrap();
        let (bytes, _) = export_bytes(&db, &ctx.id, "pw123").unwrap();

        let db2 = Database::open_memory().unwrap();
        let target = Context::from_title("Target Context");
        db2.upsert_context(&target).unwrap();

        let result = import_bytes(&db2, &bytes, "pw123", Some(&target.id)).unwrap();
        assert_eq!(result.context.id, target.id);
        assert_eq!(result.entries_imported, 2);

        let entries = db2.list_entries(&target.id).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.context_id == target.id));
        // Original context must NOT be created
        assert!(db2.get_context(&ctx.id).unwrap().is_none());

        // Unknown target context errors
        let err = import_bytes(&db2, &bytes, "pw123", Some("ctx_nonexistent")).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_wrong_password() {
        let (db, ctx) = setup_db();
        db.add_entry(&Entry::new(&ctx.id, "secret", "note")).unwrap();

        let (bytes, _) = export_bytes(&db, &ctx.id, "correct").unwrap();
        let db2 = Database::open_memory().unwrap();
        let err = import_bytes(&db2, &bytes, "wrong", None).unwrap_err();
        assert!(err.to_string().contains("wrong password"));
    }

    #[test]
    fn test_bad_magic() {
        let db = Database::open_memory().unwrap();
        let fake = vec![b'X'; 64];
        let err = import_bytes(&db, &fake, "pw", None).unwrap_err();
        assert!(err.to_string().contains("bad magic"));
    }

    #[test]
    fn test_base64_roundtrip() {
        for data in [&b""[..], b"a", b"ab", b"abc", b"abcd", &[0u8, 159, 255, 7, 66][..]] {
            let enc = b64_encode(data);
            let dec = b64_decode(&enc).unwrap();
            assert_eq!(dec, data);
        }
        // Known vectors
        assert_eq!(b64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(b64_decode("aGVsbG8=").unwrap(), b"hello");
    }
}
