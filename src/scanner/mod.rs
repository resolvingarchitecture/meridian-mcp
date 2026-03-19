use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod adrs;
pub mod imports;
pub mod patterns;
pub mod walker;

// ── ArchModel — shared with Java backend via JSON contract ───────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchModel {
    pub layers:      Vec<String>,
    pub layer_order: Vec<String>,
    pub style:       String,
    pub patterns:    Vec<String>,
    pub adrs:        Vec<String>,
    pub root:        String,
    pub scanned_at:  u64,
}

// ── Known layer names — used to identify architectural layers from dir names ──

pub const KNOWN_LAYERS: &[&str] = &[
    "controllers", "controller",
    "services",    "service",
    "domain",
    "application",
    "repositories","repository",
    "infra",       "infrastructure",
    "adapters",    "ports",
    "handlers",    "usecases", "usecase",
];

pub const IGNORED_DIRS: &[&str] = &[
    "node_modules", ".git",   "dist",   "build",
    ".next",        "coverage","target", ".idea",
    ".vscode",      "out",    "bin",    "obj",
];

// ── Main scan entry point ─────────────────────────────────────────────────────

pub fn scan(root: &Path) -> Result<ArchModel> {
    let all_paths = walker::collect_paths(root);
    let layers    = walker::infer_layers(&all_paths);
    let edges     = imports::parse_import_graph(root, &layers)?;
    let layer_order = imports::topo_sort(&edges, &layers);
    let patterns  = patterns::detect(root, &all_paths);
    let adrs      = adrs::harvest(root);
    let style     = infer_style(&layers, &patterns);

    Ok(ArchModel {
        layers,
        layer_order,
        style,
        patterns,
        adrs,
        root: root.to_string_lossy().to_string(),
        scanned_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}

pub fn infer_style(layers: &[String], patterns: &[String]) -> String {
    let has_domain  = layers.iter().any(|l| l == "domain");
    let has_ports   = layers.iter().any(|l| l == "ports" || l == "adapters");
    let has_app     = layers.iter().any(|l| l == "application");
    let has_repo    = patterns.iter().any(|p| p == "repository_pattern");

    if has_domain && has_ports {
        "hexagonal".to_string()
    } else if has_domain && has_app {
        "clean_architecture".to_string()
    } else if has_domain && has_repo {
        "layered_ddd".to_string()
    } else if has_domain {
        "layered".to_string()
    } else {
        "modular".to_string()
    }
}
