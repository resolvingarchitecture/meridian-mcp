use crate::models::{
    ContentEncoding, ContentType, DocumentContent, DocumentInput, DocumentTypeHint,
};
use crate::scanner::adrs;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Known architecture documentation files.
const ARCH_DOCS: &[&str] = &[
    "ARCHITECTURE.md",
    "architecture.md",
    "DESIGN.md",
    "design.md",
];

/// Harvest all supported architecture/context documents from the project.
pub fn harvest(paths: &[PathBuf]) -> Vec<DocumentInput> {
    let mut documents = Vec::new();

    documents.extend(adrs::harvest_from_paths(paths));
    documents.extend(harvest_architecture_docs(paths));

    documents
}

pub fn document_input_for_path(path: &Path, content: &str) -> Option<DocumentInput> {
    if !path.is_file() {
        return None;
    }

    if adrs::is_adr_path(path) {
        return Some(new_document_input(
            path,
            title_from_markdown(content).unwrap_or_else(|| "Untitled ADR".to_string()),
            DocumentTypeHint::ArchitectureDecisionRecord,
            Some("Architecture Decision Record discovered during local scan".to_string()),
            ContentType::Text,
            "text/plain",
            content.to_string(),
        ));
    }

    if is_architecture_doc_path(path) {
        let file_name = path.file_name()?.to_str()?;
        return Some(new_document_input(
            path,
            title_from_markdown(content).unwrap_or_else(|| file_name.to_string()),
            DocumentTypeHint::ApplicationDesign,
            Some("Architecture document discovered during local scan".to_string()),
            ContentType::Text,
            "text/plain",
            content.to_string(),
        ));
    }

    None
}

pub fn new_document_input(
    path: &Path,
    title: String,
    type_hint: DocumentTypeHint,
    stated_scope: Option<String>,
    content_type: ContentType,
    media_type: &str,
    data: String,
) -> DocumentInput {
    let content = new_document_content(content_type, media_type, data);
    let document_hash = aggregate_document_hash(std::slice::from_ref(&content));

    DocumentInput {
        id: document_id_for_path(path),
        title,
        filename: Some(path.to_string_lossy().to_string()),
        type_hint,
        author: None,
        date: None,
        version: None,
        stated_scope,
        organization_context: None,
        known_stakeholders: Vec::new(),
        known_decisions: Vec::new(),
        content: vec![content],
        data_hash: document_hash,
        data_hash_algorithm: "SHA-256".to_string(),
        scanned_at: Some(current_instant_string()),
    }
}

pub fn new_document_content(
    content_type: ContentType,
    media_type: &str,
    data: String,
) -> DocumentContent {
    DocumentContent {
        content_type,
        media_type: Some(media_type.to_string()),
        encoding: Some(ContentEncoding::Utf8),
        data_hash: content_hash(&data),
        data_hash_algorithm: "SHA-256".to_string(),
        data,
    }
}

pub fn aggregate_document_hash(content: &[DocumentContent]) -> String {
    let mut hasher = Sha256::new();

    for item in content {
        hasher.update(item.data_hash_algorithm.as_bytes());
        hasher.update(b":");
        hasher.update(item.data_hash.as_bytes());
        hasher.update(b"\n");
    }

    format!("{:x}", hasher.finalize())
}

pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn current_instant_string() -> String {
    humantime::format_rfc3339(std::time::SystemTime::now()).to_string()
}

pub fn title_from_markdown(content: &str) -> Option<String> {
    content
        .lines()
        .find(|line| line.starts_with("# "))
        .map(|line| line.trim_start_matches("# ").trim().to_string())
}

fn harvest_architecture_docs(paths: &[PathBuf]) -> Vec<DocumentInput> {
    let mut documents = Vec::new();

    for path in paths {
        if !is_architecture_doc_path(path) {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(path) {
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("architecture document");

            let title = title_from_markdown(&content).unwrap_or_else(|| file_name.to_string());
            let summary = format!("{file_name}: {title}");

            documents.push(new_document_input(
                path,
                title,
                DocumentTypeHint::ApplicationDesign,
                Some(summary),
                ContentType::Text,
                "text/plain",
                content,
            ));
        }
    }

    documents
}

fn is_architecture_doc_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    ARCH_DOCS.iter().any(|known| known == &file_name)
}

fn document_id_for_path(path: &Path) -> String {
    let normalized = path.to_string_lossy();
    let slug: String = normalized
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let trimmed = slug.trim_matches('-');

    if trimmed.is_empty() {
        "document".to_string()
    } else {
        format!("document-{trimmed}")
    }
}
