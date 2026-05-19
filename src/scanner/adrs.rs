use std::path::Path;

/// Known locations where teams store Architecture Decision Records.
const ADR_DIRS: &[&str] = &[
    "docs/adr",
    "docs/decisions",
    "docs/architecture",
    "adr",
    "architecture",
    ".adr",
];

/// Known architecture documentation files.
const ARCH_DOCS: &[&str] = &[
    "ARCHITECTURE.md",
    "architecture.md",
    "DESIGN.md",
    "design.md",
];

/// Harvest ADR titles and statuses from the project.
/// Reads only the first 8 lines of each file to stay lean.
pub fn harvest(root: &Path) -> Vec<String> {
    let mut adrs = vec![];

    // Scan known ADR directories
    for dir in ADR_DIRS {
        let full = root.join(dir);
        if !full.is_dir() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&full) {
            let mut files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .map(|x| x == "md")
                        .unwrap_or(false)
                })
                .collect();

            // Sort for stable ordering
            files.sort_by_key(|e| e.file_name());

            for entry in files {
                if let Some(summary) = summarise_adr(&entry.path()) {
                    adrs.push(summary);
                }
            }
        }
    }

    // Also check root-level architecture docs
    for doc in ARCH_DOCS {
        let full = root.join(doc);
        if full.exists() {
            if let Some(summary) = summarise_doc(&full) {
                adrs.push(summary);
            }
        }
    }

    // Fall back: scan for inline ARCH: or DECISION: comments in source
    if adrs.is_empty() {
        adrs.extend(scan_inline_decisions(root));
    }

    adrs
}

/// Extract title and status from an ADR markdown file.
/// Reads only first 8 lines — we want the title and status, not the full doc.
fn summarise_adr(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().take(8).collect();

    // Extract title (first # heading)
    let title = lines
        .iter()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim())
        .unwrap_or("Untitled ADR");

    // Extract status if present
    let status = lines
        .iter()
        .find(|l| {
            let lower = l.to_lowercase();
            lower.contains("status:")
                || lower.contains("accepted")
                || lower.contains("proposed")
                || lower.contains("deprecated")
        })
        .map(|l| l.trim())
        .unwrap_or("accepted"); // assume accepted if no status line

    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("adr");

    Some(format!("{filename}: {title} ({status})"))
}

fn summarise_doc(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let first_heading = content
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim().to_string())?;

    let filename = path.file_name()?.to_str()?;
    Some(format!("{filename}: {first_heading}"))
}

/// Last resort: find ARCH: or DECISION: comments inline in source files.
fn scan_inline_decisions(root: &Path) -> Vec<String> {
    let mut decisions = vec![];
    let walker = ignore::WalkBuilder::new(root).git_ignore(true).build();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
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
                    decisions.push(decision);
                    if decisions.len() >= 10 {
                        return decisions;
                    }
                }
            }
        }
    }
    decisions
}

fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "js" | "java" | "py" | "go" | "rs")
    )
}
