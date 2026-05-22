use crate::models::DocumentInput;
use anyhow::Result;
use std::path::Path;

pub mod adrs;
pub mod documents;
pub mod walker;

// ── Main scan entry point ─────────────────────────────────────────────────────

pub fn scan(root: &Path) -> Result<Vec<DocumentInput>> {
    let paths = walker::collect_paths(root);
    Ok(documents::harvest(&paths))
}

pub fn document_input_for_path(path: &Path, content: &str) -> Option<DocumentInput> {
    documents::document_input_for_path(path, content)
}
