use crate::scanner::walker::sample_source_files;
use std::path::{Path, PathBuf};

/// Detect common architectural patterns by sampling up to 40 source files.
/// Looks for structural signatures — not deep semantic analysis.
pub fn detect(_root: &Path, all_paths: &[PathBuf]) -> Vec<String> {
    let files = sample_source_files(all_paths, 40);
    let combined: String = files.iter()
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .collect::<Vec<_>>()
        .join("\n");

    let mut patterns = vec![];

    // Repository pattern
    if combined.contains("Repository")
        && (combined.contains("interface I") || combined.contains("implements"))
    {
        patterns.push("repository_pattern".to_string());
    }

    // Constructor injection
    if combined.contains("constructor(private")
        || combined.contains("constructor(")
            && combined.contains("private final")
    {
        patterns.push("constructor_injection".to_string());
    }

    // Value objects in domain
    if combined.contains("value object")
        || combined.contains("ValueObject")
        || (combined.contains("record ") && combined.contains("domain"))
    {
        patterns.push("value_objects".to_string());
    }

    // DTOs at boundary
    if combined.contains("Dto") || combined.contains("DTO") || combined.contains("Request") {
        patterns.push("dto_boundary".to_string());
    }

    // CQRS
    if combined.contains("Command") && combined.contains("Query")
        && (combined.contains("Handler") || combined.contains("Bus"))
    {
        patterns.push("cqrs".to_string());
    }

    // Event-driven
    if (combined.contains("Event") || combined.contains("event"))
        && (combined.contains("publish") || combined.contains("dispatch")
            || combined.contains("emit"))
    {
        patterns.push("domain_events".to_string());
    }

    // Factory pattern
    if combined.contains("Factory") && combined.contains("create(") {
        patterns.push("factory_pattern".to_string());
    }

    // Anti-corruption layer
    if combined.contains("AntiCorruption")
        || combined.contains("Translator")
        || combined.contains("Mapper")
    {
        patterns.push("anti_corruption_layer".to_string());
    }

    patterns
}
