use crate::scanner::walker::get_layer;
use anyhow::Result;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ImportEdge {
    pub from_layer: String,
    pub to_layer:   String,
}

/// Parse import statements across the project using tree-sitter.
/// Uses rayon for parallel file processing.
pub fn parse_import_graph(root: &Path, layers: &[String]) -> Result<Vec<ImportEdge>> {
    // Collect only source files that matter for import analysis
    let files: Vec<PathBuf> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| is_source_file(e.path()))
        .map(|e| e.into_path())
        .collect();

    // Parse in parallel — each file is independent
    let edges: Vec<ImportEdge> = files
        .par_iter()
        .flat_map(|file| extract_edges(file, layers).unwrap_or_default())
        .collect();

    Ok(edges)
}

/// Topological sort of layers based on import direction.
/// Returns layers ordered from most-depended-on (top) to least (bottom).
pub fn topo_sort(edges: &[ImportEdge], layers: &[String]) -> Vec<String> {
    let mut graph = DiGraph::new();
    let mut indices: HashMap<String, NodeIndex> = HashMap::new();

    for layer in layers {
        let idx = graph.add_node(layer.clone());
        indices.insert(layer.clone(), idx);
    }

    for edge in edges {
        if let (Some(&a), Some(&b)) = (
            indices.get(&edge.from_layer),
            indices.get(&edge.to_layer),
        ) {
            graph.add_edge(a, b, ());
        }
    }

    match toposort(&graph, None) {
        Ok(sorted) => sorted.iter().map(|i| graph[*i].clone()).collect(),
        Err(_) => {
            // Cycle detected — return as-is rather than failing
            tracing::warn!("Cycle detected in import graph — layer order may be inaccurate");
            layers.to_vec()
        }
    }
}

// ── Per-file import extraction ────────────────────────────────────────────────

fn extract_edges(file: &Path, layers: &[String]) -> Result<Vec<ImportEdge>> {
    let source = std::fs::read_to_string(file)?;
    let from_layer = match get_layer(file, layers) {
        Some(l) => l,
        None    => return Ok(vec![]),
    };

    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
    let import_paths = match ext {
        "ts" | "tsx"       => extract_ts_imports(&source)?,
        "js" | "jsx"       => extract_js_imports(&source)?,
        "java"             => extract_java_imports(&source),
        "py"               => extract_python_imports(&source),
        _                  => vec![],
    };

    let edges = import_paths.iter()
        .filter_map(|imp| get_layer_from_import(imp, layers))
        .filter(|to| to != &from_layer)
        .map(|to_layer| ImportEdge {
            from_layer: from_layer.clone(),
            to_layer,
        })
        .collect();

    Ok(edges)
}

fn extract_ts_imports(source: &str) -> Result<Vec<String>> {
    use tree_sitter::Parser;

    let language = tree_sitter_typescript::language_typescript();
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let tree = parser.parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter parse failed"))?;

    let query = tree_sitter::Query::new(
        &language,
        r#"(import_declaration source: (string (string_fragment) @path))"#,
    )?;

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut paths = vec![];

    for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        for cap in m.captures {
            if let Ok(text) = cap.node.utf8_text(source.as_bytes()) {
                paths.push(text.to_string());
            }
        }
    }
    Ok(paths)
}

fn extract_js_imports(source: &str) -> Result<Vec<String>> {
    use tree_sitter::Parser;

    let language = tree_sitter_javascript::language();
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let tree = parser.parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter parse failed"))?;

    let query = tree_sitter::Query::new(
        &language,
        r#"(import_declaration source: (string (string_fragment) @path))"#,
    )?;

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut paths = vec![];

    for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        for cap in m.captures {
            if let Ok(text) = cap.node.utf8_text(source.as_bytes()) {
                paths.push(text.to_string());
            }
        }
    }
    Ok(paths)
}

fn extract_java_imports(source: &str) -> Vec<String> {
    // Java imports are simple enough for regex — no need for full AST
    source.lines()
        .filter(|l| l.trim_start().starts_with("import "))
        .map(|l| l.trim().to_string())
        .collect()
}

fn extract_python_imports(source: &str) -> Vec<String> {
    source.lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("import ") || t.starts_with("from ")
        })
        .map(|l| l.trim().to_string())
        .collect()
}

fn get_layer_from_import(import: &str, layers: &[String]) -> Option<String> {
    for layer in layers {
        if import.contains(&format!("/{layer}/"))
            || import.contains(&format!(".{layer}."))
            || import.contains(&format!("/{layer}\""))
            || import.ends_with(layer.as_str())
        {
            return Some(layer.clone());
        }
    }
    None
}

fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "tsx" | "js" | "jsx" | "java" | "py" | "go" | "rs")
    )
}
