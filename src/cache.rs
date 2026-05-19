use crate::scanner::ArchModel;
use anyhow::Result;
use sha2::{Digest, Sha256};
use sled::Db;
use std::path::Path;
use std::sync::OnceLock;

static DB: OnceLock<Db> = OnceLock::new();

fn db() -> Result<&'static Db> {
    let db = DB.get_or_init(|| {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
            .join("meridian");
        sled::open(&cache_dir).expect("failed to open cache at ~/.cache/meridian")
    });
    Ok(db)
}

/// Retrieve a cached ArchModel for the given project root.
/// Returns None if not cached or if the cache entry is stale.
pub fn get(root: &str) -> Result<Option<ArchModel>> {
    let key = cache_key(root)?;
    match db()?.get(&key)? {
        None => Ok(None),
        Some(bytes) => {
            let model: ArchModel = serde_json::from_slice(&bytes)?;

            // Invalidate if older than 30 minutes
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now.saturating_sub(model.scanned_at) > 1800 {
                tracing::debug!("cache stale for {root} — will rescan");
                return Ok(None);
            }

            Ok(Some(model))
        }
    }
}

/// Store an ArchModel in the cache.
pub fn set(root: &str, model: &ArchModel) -> Result<()> {
    let key = cache_key(root)?;
    let bytes = serde_json::to_vec(model)?;
    db()?.insert(key, bytes)?;
    Ok(())
}

/// Remove the cached model for a project root.
pub fn invalidate(root: &str) -> Result<()> {
    let key = cache_key(root)?;
    db()?.remove(key)?;
    Ok(())
}

/// Cache key = SHA-256 of the root path + directory structure hash.
/// Automatically invalidates when directories are added/removed.
fn cache_key(root: &str) -> Result<Vec<u8>> {
    let mut hasher = Sha256::new();
    hasher.update(root.as_bytes());

    // Hash the directory structure (names only, not contents)
    if let Ok(entries) = dir_fingerprint(Path::new(root)) {
        hasher.update(entries.as_bytes());
    }

    Ok(hasher.finalize().to_vec())
}

/// Build a stable string fingerprint of the directory structure.
/// Only looks at names and structure — not file contents.
fn dir_fingerprint(root: &Path) -> Result<String> {
    let mut paths: Vec<String> = ignore::WalkBuilder::new(root)
        .git_ignore(true)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| {
            e.path()
                .strip_prefix(root)
                .unwrap_or(e.path())
                .to_string_lossy()
                .to_string()
        })
        .collect();

    paths.sort();
    Ok(paths.join("|"))
}
