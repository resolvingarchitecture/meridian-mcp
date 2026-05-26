use crate::models::CachedArchitectureReviewRequest;
use anyhow::Result;
use sled::Db;
use std::sync::OnceLock;

static DB: OnceLock<Db> = OnceLock::new();

const ARCHITECTURE_REVIEW_REQUEST_KEY: &[u8] = b"architecture-review-request";

fn db() -> Result<&'static Db> {
    let db = DB.get_or_init(|| {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
            .join("meridian");
        sled::open(&cache_dir).expect("failed to open cache at ~/.cache/meridian")
    });
    Ok(db)
}

/// Retrieve the cached ArchitectureReviewRequest template.
pub fn get() -> Result<Option<CachedArchitectureReviewRequest>> {
    match db()?.get(ARCHITECTURE_REVIEW_REQUEST_KEY)? {
        None => Ok(None),
        Some(bytes) => {
            let cached: CachedArchitectureReviewRequest = serde_json::from_slice(&bytes)?;
            Ok(Some(cached))
        }
    }
}

/// Store the ArchitectureReviewRequest template.
pub fn set(cached: &CachedArchitectureReviewRequest) -> Result<()> {
    let bytes = serde_json::to_vec(cached)?;
    db()?.insert(ARCHITECTURE_REVIEW_REQUEST_KEY, bytes)?;
    db()?.flush()?;
    Ok(())
}

/// Remove the cached ArchitectureReviewRequest template.
pub fn invalidate() -> Result<()> {
    db()?.remove(ARCHITECTURE_REVIEW_REQUEST_KEY)?;
    db()?.flush()?;
    Ok(())
}
