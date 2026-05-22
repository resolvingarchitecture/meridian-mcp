use crate::models::{ContentType, DocumentInput, DocumentTypeHint};
use crate::scanner::documents;
use std::path::{Path, PathBuf};

/// Known locations where teams store Architecture Decision Records.
const ADR_DIRS: &[&str] = &[
    "docs/adr",
    "docs/decisions",
    "docs/architecture",
    "adr",
    "architecture",
    ".adr",
];

/// Harvest ADR documents from already-collected project paths.
pub fn harvest_from_paths(paths: &[PathBuf]) -> Vec<DocumentInput> {
    let mut adrs = Vec::new();

    for path in paths {
        if is_markdown_file(path) && is_adr_path(path) {
            if let Some(document) = summarise_adr(path) {
                adrs.push(document);
            }
        }
    }

    adrs.sort_by(|a, b| a.filename.cmp(&b.filename));

    if adrs.is_empty() {
        adrs.extend(scan_inline_decisions_from_paths(paths));
    }

    adrs
}

pub fn is_adr_path(path: &Path) -> bool {
    let path_text = path.to_string_lossy().replace('\\', "/");
    ADR_DIRS
        .iter()
        .any(|known_dir| path_text.contains(known_dir))
}

/// Extract title and status from an ADR markdown file.
fn summarise_adr(path: &Path) -> Option<DocumentInput> {
    let content = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().take(8).collect();

    // Extract title (first # heading)
    let title = lines
        .iter()
        .find(|line| line.starts_with("# "))
        .map(|line| line.trim_start_matches("# ").trim().to_string())
        .unwrap_or_else(|| "Untitled ADR".to_string());

    // Extract status if present
    let status = lines
        .iter()
        .find(|line| {
            let lower = line.to_lowercase();
            lower.contains("status:")
                || lower.contains("accepted")
                || lower.contains("proposed")
                || lower.contains("deprecated")
        })
        .map(|line| line.trim())
        .unwrap_or("accepted");

    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("adr");
    let summary = format!("{filename}: {title} ({status})");

    Some(documents::new_document_input(
        path,
        title,
        DocumentTypeHint::ArchitectureDecisionRecord,
        Some(summary),
        ContentType::Text,
        "text/plain",
        content,
    ))
}

/// Last resort: find ARCH: or DECISION: comments inline in source files.
fn scan_inline_decisions_from_paths(paths: &[PathBuf]) -> Vec<DocumentInput> {
    let mut decisions = Vec::new();

    for path in paths {
        if !is_source_file(path) {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.contains("ARCH:") || trimmed.contains("DECISION:") {
                    let decision = trimmed
                        .trim_start_matches("//")
                        .trim_start_matches("*")
                        .trim_start_matches("#")
                        .trim()
                        .to_string();

                    decisions.push(documents::new_document_input(
                        path,
                        "Inline architecture decision".to_string(),
                        DocumentTypeHint::ArchitectureDecisionRecord,
                        Some(decision.clone()),
                        ContentType::Text,
                        "text/plain",
                        decision,
                    ));

                    if decisions.len() >= 10 {
                        return decisions;
                    }
                }
            }
        }
    }

    decisions
}

fn is_markdown_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("md"))
}

fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "js" | "java" | "py" | "go" | "rs")
    )
}
